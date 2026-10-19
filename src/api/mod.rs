//! The HTTP API (`/api/v1`) and the router that also serves the web UI.

mod error;
mod messages;
mod stream;

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderValue, Method},
    routing::{get, post},
};
use serde::Serialize;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};

pub use error::ApiError;

use crate::{Mailroom, web};

/// Static facts about the running server, reported by `GET /api/v1/info`.
#[derive(Debug, Clone, Serialize)]
pub struct ServerInfo {
    pub version: &'static str,
    pub smtp_addr: String,
    pub http_addr: String,
    pub hostname: String,
    pub smtp_auth_required: bool,
    pub smtp_tls: bool,
    pub smtp_require_tls: bool,
    pub database: Option<String>,
    pub max_messages: usize,
    pub max_message_size: usize,
}

#[derive(Clone)]
pub struct ApiState {
    pub app: Mailroom,
    pub info: Arc<ServerInfo>,
}

pub fn router(app: Mailroom, info: ServerInfo, cors: &[String]) -> Router {
    let webroot = app.options().webroot.clone();
    let state = ApiState {
        app,
        info: Arc::new(info),
    };

    let mut api = Router::new()
        .route(
            "/messages",
            get(messages::list).delete(messages::delete_many),
        )
        .route("/messages/read", post(messages::mark_read))
        .route("/messages/wait", get(stream::wait))
        .route(
            "/messages/{id}",
            get(messages::detail)
                .patch(messages::update)
                .delete(messages::delete_one),
        )
        .route("/messages/{id}/raw", get(messages::raw))
        .route("/messages/{id}/html", get(messages::html))
        .route("/messages/{id}/parts/{part}", get(messages::part))
        .route("/messages/{id}/links", get(messages::links))
        .route("/messages/{id}/codes", get(messages::codes))
        .route("/send", post(messages::send))
        .route("/recipients", get(messages::recipients))
        .route("/events", get(stream::events))
        .route("/info", get(server_info));
    if let Some(layer) = cors_layer(cors) {
        api = api.layer(layer);
    }

    // Routes are registered with their full paths rather than nested, so the
    // UI is reachable at exactly the webroot (`/mail/`, not only `/mail`).
    let api_prefix = format!("{webroot}api/v1");
    Router::new()
        .nest(&api_prefix, api)
        .route(&format!("{webroot}healthz"), get(|| async { "ok" }))
        .merge(web::routes(&webroot))
        .with_state(state)
}

fn cors_layer(origins: &[String]) -> Option<CorsLayer> {
    if origins.is_empty() {
        return None;
    }
    let allow_origin = if origins.iter().any(|o| o == "*") {
        AllowOrigin::from(Any)
    } else {
        AllowOrigin::list(
            origins
                .iter()
                .filter_map(|o| HeaderValue::from_str(o.trim()).ok()),
        )
    };
    Some(
        CorsLayer::new()
            .allow_origin(allow_origin)
            .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
            .allow_headers(Any),
    )
}

#[derive(Serialize)]
struct InfoResponse {
    #[serde(flatten)]
    info: ServerInfo,
    stats: crate::storage::Stats,
    uptime_seconds: u64,
}

async fn server_info(State(state): State<ApiState>) -> Result<Json<InfoResponse>, ApiError> {
    Ok(Json(InfoResponse {
        info: (*state.info).clone(),
        stats: state.app.stats().await?,
        uptime_seconds: state.app.uptime().as_secs(),
    }))
}
