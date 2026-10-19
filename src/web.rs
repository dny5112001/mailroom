//! The embedded single-page web UI.
//!
//! Assets are compiled into the binary. Set `MAILROOM_UI_DIR` to a directory
//! containing them (normally `ui/`) to serve them from disk instead, so UI
//! changes show up on reload without rebuilding.

use std::path::PathBuf;

use axum::{
    Router,
    extract::Path,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
    routing::get,
};

use crate::api::ApiState;

struct Asset {
    name: &'static str,
    content_type: &'static str,
    body: &'static str,
}

const ASSETS: &[Asset] = &[
    Asset {
        name: "app.js",
        content_type: "text/javascript; charset=utf-8",
        body: include_str!("../ui/app.js"),
    },
    Asset {
        name: "app.css",
        content_type: "text/css; charset=utf-8",
        body: include_str!("../ui/app.css"),
    },
    Asset {
        name: "favicon.svg",
        content_type: "image/svg+xml",
        body: include_str!("../ui/favicon.svg"),
    },
];

const INDEX: &str = include_str!("../ui/index.html");

pub fn routes(webroot: &str) -> Router<ApiState> {
    let root = webroot.to_string();
    let mut router = Router::new()
        .route(
            webroot,
            get(move || {
                let root = root.clone();
                async move { index(&root).await }
            }),
        )
        .route(&format!("{webroot}assets/{{name}}"), get(asset));
    if webroot != "/" {
        let target = webroot.to_string();
        router = router.route(
            webroot.trim_end_matches('/'),
            get(move || async move { Redirect::permanent(&target) }),
        );
    }
    router
}

fn ui_dir() -> Option<PathBuf> {
    std::env::var_os("MAILROOM_UI_DIR").map(PathBuf::from)
}

async fn load(name: &str, embedded: &'static str) -> String {
    if let Some(dir) = ui_dir() {
        if let Ok(body) = tokio::fs::read_to_string(dir.join(name)).await {
            return body;
        }
    }
    embedded.to_string()
}

async fn index(webroot: &str) -> Response {
    let html = load("index.html", INDEX)
        .await
        .replace("{{WEBROOT}}", webroot)
        .replace("{{VERSION}}", env!("CARGO_PKG_VERSION"));
    (
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/html; charset=utf-8"),
            ),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-cache")),
        ],
        html,
    )
        .into_response()
}

async fn asset(Path(name): Path<String>) -> Response {
    let Some(asset) = ASSETS.iter().find(|a| a.name == name) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let cache = if ui_dir().is_some() {
        "no-cache"
    } else {
        // URLs carry the version, so a release can be cached aggressively.
        "public, max-age=31536000, immutable"
    };
    (
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static(asset.content_type),
            ),
            (header::CACHE_CONTROL, HeaderValue::from_static(cache)),
        ],
        load(asset.name, asset.body).await,
    )
        .into_response()
}
