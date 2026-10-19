//! Message endpoints: list, read, update, delete, download and send.

use axum::{
    Json,
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{ApiError, ApiState};
use crate::{
    Mailroom,
    email::{MessageDetail, MessageSummary, compose::Draft},
    storage::{SearchQuery, Selection},
};

type ApiResult<T> = Result<T, ApiError>;

const DEFAULT_LIMIT: usize = 50;
const MAX_LIMIT: usize = 1000;

#[derive(Debug, Default, Deserialize)]
pub struct ListParams {
    #[serde(alias = "q")]
    query: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

#[derive(Serialize)]
pub struct ListResponse {
    /// Messages in the mailbox.
    total: u64,
    unread: u64,
    /// Messages matching the query.
    matched: u64,
    offset: usize,
    limit: usize,
    messages: Vec<MessageSummary>,
}

pub async fn list(
    State(state): State<ApiState>,
    Query(params): Query<ListParams>,
) -> ApiResult<Json<ListResponse>> {
    let query = SearchQuery::parse(params.query.as_deref().unwrap_or(""));
    let limit = params.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let offset = params.offset.unwrap_or(0);
    let (messages, matched) = state.app.list(query, limit, offset).await?;
    let stats = state.app.stats().await?;
    Ok(Json(ListResponse {
        total: stats.total,
        unread: stats.unread,
        matched,
        offset,
        limit,
        messages,
    }))
}

/// Query parameters accepted by every `/messages/{id}/...` endpoint.
#[derive(Debug, Default, Deserialize)]
pub struct ItemParams {
    /// With the id `latest`, pick the newest message matching this query.
    #[serde(alias = "q")]
    query: Option<String>,
    download: Option<String>,
    block_remote: Option<String>,
}

/// Resolve the special id `latest` to the newest message matching `query`.
async fn resolve(app: &Mailroom, id: &str, params: &ItemParams) -> ApiResult<String> {
    if !id.eq_ignore_ascii_case("latest") {
        return Ok(id.to_string());
    }
    let query = SearchQuery::parse(params.query.as_deref().unwrap_or(""));
    match app.latest(query).await? {
        Some(summary) => Ok(summary.id),
        None => Err(ApiError::not_found("no message matches")),
    }
}

async fn load_detail(app: &Mailroom, id: &str, params: &ItemParams) -> ApiResult<MessageDetail> {
    let id = resolve(app, id, params).await?;
    app.detail(&id)
        .await?
        .ok_or_else(ApiError::message_not_found)
}

pub async fn detail(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Query(params): Query<ItemParams>,
) -> ApiResult<Json<MessageDetail>> {
    Ok(Json(load_detail(&state.app, &id, &params).await?))
}

#[derive(Debug, Deserialize)]
pub struct UpdateBody {
    read: Option<bool>,
}

pub async fn update(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Query(params): Query<ItemParams>,
    Json(body): Json<UpdateBody>,
) -> ApiResult<Json<MessageSummary>> {
    let id = resolve(&state.app, &id, &params).await?;
    if let Some(read) = body.read {
        state
            .app
            .set_read(Selection::Ids(vec![id.clone()]), read)
            .await?;
    }
    let summary = state
        .app
        .summary(&id)
        .await?
        .ok_or_else(ApiError::message_not_found)?;
    Ok(Json(summary))
}

#[derive(Debug, Default, Deserialize)]
pub struct MarkReadBody {
    ids: Option<Vec<String>>,
    query: Option<String>,
    read: Option<bool>,
}

/// `POST /messages/read` — mark messages (by ids, by query, or all) read or unread.
pub async fn mark_read(
    State(state): State<ApiState>,
    body: Bytes,
) -> ApiResult<Json<serde_json::Value>> {
    let body: MarkReadBody = parse_optional_json(&body)?;
    let selection = selection(body.ids, body.query);
    let updated = state
        .app
        .set_read(selection, body.read.unwrap_or(true))
        .await?;
    Ok(Json(json!({ "updated": updated.len() })))
}

#[derive(Debug, Default, Deserialize)]
pub struct DeleteBody {
    ids: Option<Vec<String>>,
}

/// `DELETE /messages` — delete by ids (JSON body), by `?query=`, or everything.
pub async fn delete_many(
    State(state): State<ApiState>,
    Query(params): Query<ListParams>,
    body: Bytes,
) -> ApiResult<Json<serde_json::Value>> {
    let body: DeleteBody = parse_optional_json(&body)?;
    let selection = selection(body.ids, params.query);
    let before = state.app.stats().await?.total;
    let deleted = match state.app.delete(selection).await? {
        Some(ids) => ids.len() as u64,
        None => before,
    };
    Ok(Json(json!({ "deleted": deleted })))
}

pub async fn delete_one(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Query(params): Query<ItemParams>,
) -> ApiResult<StatusCode> {
    let id = resolve(&state.app, &id, &params).await?;
    match state.app.delete(Selection::Ids(vec![id])).await? {
        Some(ids) if !ids.is_empty() => Ok(StatusCode::NO_CONTENT),
        _ => Err(ApiError::message_not_found()),
    }
}

/// `GET /messages/{id}/raw` — the original message source.
pub async fn raw(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Query(params): Query<ItemParams>,
) -> ApiResult<Response> {
    let id = resolve(&state.app, &id, &params).await?;
    let raw = state
        .app
        .raw(&id)
        .await?
        .ok_or_else(ApiError::message_not_found)?;
    let mut headers = HeaderMap::new();
    if truthy(&params.download) {
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("message/rfc822"),
        );
        let filename = match state.app.summary(&id).await? {
            Some(summary) if !summary.subject.is_empty() => format!("{}.eml", summary.subject),
            _ => format!("{id}.eml"),
        };
        headers.insert(
            header::CONTENT_DISPOSITION,
            content_disposition("attachment", &filename),
        );
    } else {
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        );
    }
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    Ok((headers, raw).into_response())
}

/// `GET /messages/{id}/html` — the HTML body as a standalone, sandboxed page
/// for the preview iframe. Scripts never run; `block_remote=1` also blocks
/// remote images, fonts and stylesheets (tracking pixels).
pub async fn html(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Query(params): Query<ItemParams>,
) -> ApiResult<Response> {
    let detail = load_detail(&state.app, &id, &params).await?;
    let html = detail
        .html
        .ok_or_else(|| ApiError::not_found("message has no HTML body"))?;

    let csp = if truthy(&params.block_remote) {
        "default-src 'none'; img-src 'self' data: blob:; style-src 'self' 'unsafe-inline'; \
         font-src 'self' data:; media-src 'self' data: blob:; form-action 'none'; \
         sandbox allow-popups allow-popups-to-escape-sandbox allow-same-origin"
    } else {
        "default-src 'none'; img-src * data: blob:; style-src * 'unsafe-inline'; font-src * data:; \
         media-src * data: blob:; form-action 'none'; \
         sandbox allow-popups allow-popups-to-escape-sandbox allow-same-origin"
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(csp),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok((headers, open_links_in_new_tab(&html)).into_response())
}

/// `GET /messages/{id}/parts/{part}` — download an attachment or inline part.
pub async fn part(
    State(state): State<ApiState>,
    Path((id, part)): Path<(String, u32)>,
    Query(params): Query<ItemParams>,
) -> ApiResult<Response> {
    let id = resolve(&state.app, &id, &params).await?;
    let content = state
        .app
        .part(&id, part)
        .await?
        .ok_or_else(|| ApiError::not_found("part not found"))?;

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&content.content_type)
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    let disposition = if truthy(&params.download) {
        "attachment"
    } else {
        "inline"
    };
    let filename = content.filename.unwrap_or_else(|| format!("part-{part}"));
    headers.insert(
        header::CONTENT_DISPOSITION,
        content_disposition(disposition, &filename),
    );
    // Attachments are untrusted: never let an HTML or SVG part run script on
    // Mailroom's origin.
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'none'; img-src 'self' data:; style-src 'unsafe-inline'; sandbox",
        ),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    Ok((headers, content.data).into_response())
}

pub async fn links(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Query(params): Query<ItemParams>,
) -> ApiResult<Json<serde_json::Value>> {
    let detail = load_detail(&state.app, &id, &params).await?;
    Ok(Json(
        json!({ "id": detail.summary.id, "links": detail.links }),
    ))
}

pub async fn codes(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Query(params): Query<ItemParams>,
) -> ApiResult<Json<serde_json::Value>> {
    let detail = load_detail(&state.app, &id, &params).await?;
    Ok(Json(
        json!({ "id": detail.summary.id, "codes": detail.codes }),
    ))
}

/// `POST /send` — compose a message from JSON and store it as if it had
/// arrived over SMTP.
pub async fn send(
    State(state): State<ApiState>,
    Json(draft): Json<Draft>,
) -> ApiResult<(StatusCode, Json<MessageSummary>)> {
    match state.app.ingest_draft(draft).await? {
        Ok(summary) => Ok((StatusCode::CREATED, Json(summary))),
        Err(message) => Err(ApiError::bad_request(message)),
    }
}

fn selection(ids: Option<Vec<String>>, query: Option<String>) -> Selection {
    if let Some(ids) = ids {
        return Selection::Ids(ids);
    }
    match query.map(|q| SearchQuery::parse(&q)) {
        Some(query) if !query.is_empty() => Selection::Query(query),
        _ => Selection::All,
    }
}

fn parse_optional_json<T: for<'de> Deserialize<'de> + Default>(body: &[u8]) -> ApiResult<T> {
    if body.iter().all(u8::is_ascii_whitespace) {
        return Ok(T::default());
    }
    serde_json::from_slice(body)
        .map_err(|err| ApiError::bad_request(format!("invalid JSON body: {err}")))
}

fn truthy(value: &Option<String>) -> bool {
    match value.as_deref() {
        None => false,
        Some(v) => !matches!(
            v.to_ascii_lowercase().as_str(),
            "0" | "false" | "no" | "off"
        ),
    }
}

/// `Content-Disposition` with an ASCII fallback and an RFC 5987 UTF-8 name.
fn content_disposition(kind: &str, filename: &str) -> HeaderValue {
    let fallback: String = filename
        .chars()
        .map(|c| {
            if c.is_ascii_graphic() && c != '"' && c != '\\' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let mut encoded = String::new();
    for byte in filename.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    HeaderValue::from_str(&format!(
        "{kind}; filename=\"{fallback}\"; filename*=UTF-8''{encoded}"
    ))
    .unwrap_or_else(|_| HeaderValue::from_static("attachment"))
}

/// Make every link in the preview open in a new tab instead of inside the
/// sandboxed frame.
fn open_links_in_new_tab(html: &str) -> String {
    const BASE: &str = r#"<base target="_blank">"#;
    let lower = html.to_ascii_lowercase();
    if let Some(start) = lower.find("<head") {
        if let Some(end) = lower[start..].find('>') {
            let at = start + end + 1;
            return format!("{}{BASE}{}", &html[..at], &html[at..]);
        }
    }
    format!("{BASE}{html}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injects_base_target() {
        assert_eq!(
            open_links_in_new_tab("<html><HEAD lang=en><title>x</title></head></html>"),
            r#"<html><HEAD lang=en><base target="_blank"><title>x</title></head></html>"#
        );
        assert_eq!(
            open_links_in_new_tab("<p>hi</p>"),
            r#"<base target="_blank"><p>hi</p>"#
        );
    }

    #[test]
    fn encodes_filenames() {
        let value = content_disposition("attachment", "résumé \"v2\".pdf");
        assert_eq!(
            value.to_str().unwrap(),
            "attachment; filename=\"r_sum_ _v2_.pdf\"; filename*=UTF-8''r%C3%A9sum%C3%A9%20%22v2%22.pdf"
        );
    }

    #[test]
    fn truthy_values() {
        assert!(truthy(&Some("1".into())));
        assert!(truthy(&Some("".into())));
        assert!(!truthy(&Some("false".into())));
        assert!(!truthy(&None));
    }
}
