//! Real-time endpoints: the SSE event stream and long-polling `wait`.

use std::{convert::Infallible, time::Duration};

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::sse::{Event as SseEvent, KeepAlive, Sse},
};
use futures_util::{Stream, StreamExt};
use serde::Deserialize;
use tokio::{sync::broadcast::error::RecvError, time::Instant};
use tokio_stream::wrappers::{BroadcastStream, errors::BroadcastStreamRecvError};

use super::{ApiError, ApiState};
use crate::{
    email::MessageDetail,
    events::Event,
    storage::{SearchQuery, query::parse_time},
};

/// `GET /events` — server-sent events for every mailbox change. A `resync`
/// event tells a slow client it missed events and should reload.
pub async fn events(
    State(state): State<ApiState>,
) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
    let stream = BroadcastStream::new(state.app.subscribe()).map(|item| {
        Ok(match item {
            Ok(event) => SseEvent::default()
                .event(event.name())
                .data(event.payload().to_string()),
            Err(BroadcastStreamRecvError::Lagged(_)) => {
                SseEvent::default().event("resync").data("{}")
            }
        })
    });
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

#[derive(Debug, Deserialize)]
pub struct WaitParams {
    #[serde(alias = "q")]
    query: Option<String>,
    /// Seconds to wait before giving up (default 30, max 300).
    timeout: Option<f64>,
    /// Only consider messages received at or after this time (RFC 3339, date,
    /// or unix seconds/milliseconds).
    since: Option<String>,
}

/// `GET /messages/wait` — return the newest message matching `query`, waiting
/// for one to arrive if none exists yet. Built for end-to-end tests:
///
/// ```text
/// GET /api/v1/messages/wait?query=to:alice@example.com subject:reset&timeout=10
/// ```
pub async fn wait(
    State(state): State<ApiState>,
    Query(params): Query<WaitParams>,
) -> Result<Json<MessageDetail>, ApiError> {
    let mut query = SearchQuery::parse(params.query.as_deref().unwrap_or(""));
    if let Some(since) = params.since.as_deref() {
        let millis = parse_time(since)
            .ok_or_else(|| ApiError::bad_request(format!("invalid `since` value {since:?}")))?;
        query = query.received_since(millis);
    }
    let timeout = Duration::from_secs_f64(params.timeout.unwrap_or(30.0).clamp(0.0, 300.0));
    let deadline = Instant::now() + timeout;

    // Subscribe before the first lookup so a message arriving in between is
    // not missed.
    let mut events = state.app.subscribe();
    loop {
        if let Some(summary) = state.app.latest(query.clone()).await?
            && let Some(detail) = state.app.detail(&summary.id).await?
        {
            return Ok(Json(detail));
        }
        loop {
            match tokio::time::timeout_at(deadline, events.recv()).await {
                Err(_) => {
                    return Err(ApiError::new(
                        StatusCode::REQUEST_TIMEOUT,
                        "timed out waiting for a matching message",
                    ));
                }
                Ok(Ok(Event::Created(_))) | Ok(Err(RecvError::Lagged(_))) => break,
                Ok(Ok(_)) => continue,
                Ok(Err(RecvError::Closed)) => {
                    return Err(ApiError::new(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "server is shutting down",
                    ));
                }
            }
        }
    }
}
