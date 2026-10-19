//! Mailroom: a local SMTP server and web inbox for testing email.
//!
//! ```text
//! TCP connection → SMTP session → envelope + raw message
//!   → MIME analysis → SQLite → HTTP API / SSE → web UI
//! ```

pub mod api;
mod app;
pub mod config;
pub mod email;
pub mod events;
pub mod sendmail;
pub mod smtp;
pub mod storage;
mod web;

use std::net::SocketAddr;

use anyhow::{Context, Result};
use tokio::{net::TcpListener, task::JoinHandle};
use tracing::info;

pub use app::{AppOptions, Mailroom, new_id};
pub use config::Config;

use crate::{
    api::ServerInfo,
    smtp::{AuthPolicy, SessionConfig, SmtpServer, tls},
    storage::Store,
};

/// A started server. Dropping it does not stop the server; call
/// [`Running::shutdown`] or [`Running::wait`].
pub struct Running {
    pub smtp_addr: SocketAddr,
    pub http_addr: SocketAddr,
    pub app: Mailroom,
    smtp: JoinHandle<()>,
    http: JoinHandle<std::io::Result<()>>,
}

impl Running {
    /// Base URL of the web UI, e.g. `http://127.0.0.1:8025/`.
    pub fn url(&self) -> String {
        format!("http://{}{}", self.http_addr, self.app.options().webroot)
    }

    /// Run until either server stops or a shutdown signal arrives.
    pub async fn wait(self) -> Result<()> {
        let Running { smtp, http, .. } = self;
        tokio::select! {
            result = http => result.context("http server task failed")?.context("http server failed"),
            _ = smtp => Ok(()),
            _ = shutdown_signal() => {
                info!("shutting down");
                Ok(())
            }
        }
    }

    pub fn shutdown(self) {
        self.smtp.abort();
        self.http.abort();
    }
}

/// Bind both listeners and start serving in the background.
pub async fn start(config: &Config) -> Result<Running> {
    let store = Store::open(config.db.as_deref())?;
    let app = Mailroom::new(
        store,
        AppOptions {
            max_messages: config.max_messages,
            webroot: config.webroot.clone(),
        },
    );

    let tls = match (&config.smtp_tls_cert, &config.smtp_tls_key) {
        (Some(cert), Some(key)) => Some(tls::acceptor_from_files(cert, key)?),
        _ if config.smtp_tls_self_signed => Some(tls::self_signed_acceptor(&config.hostname)?),
        _ => None,
    };
    if config.smtp_require_tls && tls.is_none() {
        anyhow::bail!(
            "--smtp-require-tls needs --smtp-tls-cert/--smtp-tls-key or --smtp-tls-self-signed"
        );
    }

    let session = SessionConfig {
        hostname: config.hostname.clone(),
        max_message_size: config.max_message_size,
        auth: match &config.smtp_auth {
            Some(creds) => AuthPolicy::Required {
                username: creds.username.clone(),
                password: creds.password.clone(),
            },
            None => AuthPolicy::AcceptAny,
        },
        require_tls: config.smtp_require_tls,
        ..Default::default()
    };

    let smtp_listener = TcpListener::bind(&config.smtp_addr)
        .await
        .with_context(|| format!("binding SMTP listener on {}", config.smtp_addr))?;
    let http_listener = TcpListener::bind(&config.http_addr)
        .await
        .with_context(|| format!("binding HTTP listener on {}", config.http_addr))?;
    let smtp_addr = smtp_listener.local_addr()?;
    let http_addr = http_listener.local_addr()?;

    let info = ServerInfo {
        version: env!("CARGO_PKG_VERSION"),
        smtp_addr: smtp_addr.to_string(),
        http_addr: http_addr.to_string(),
        hostname: config.hostname.clone(),
        smtp_auth_required: config.smtp_auth.is_some(),
        smtp_tls: tls.is_some(),
        smtp_require_tls: config.smtp_require_tls,
        database: config.db.as_ref().map(|p| p.display().to_string()),
        max_messages: config.max_messages,
        max_message_size: config.max_message_size,
    };
    let router = api::router(app.clone(), info, &config.cors);

    let smtp = tokio::spawn(SmtpServer::new(app.clone(), session, tls).serve(smtp_listener));
    let http = tokio::spawn(async move { axum::serve(http_listener, router).await });

    Ok(Running {
        smtp_addr,
        http_addr,
        app,
        smtp,
        http,
    })
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
