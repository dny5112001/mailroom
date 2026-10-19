//! The core service shared by the SMTP server and the HTTP API.

use std::{net::SocketAddr, sync::Arc, time::Instant};

use anyhow::Result;
use chrono::Utc;
use tokio::sync::broadcast;

use crate::{
    email::{
        Envelope, MessageDetail, MessageSummary,
        compose::{self, Draft},
        parser::{self, PartContent},
    },
    events::Event,
    smtp,
    storage::{Recipient, SearchQuery, Selection, Stats, Store, repo},
};

#[derive(Debug, Clone)]
pub struct AppOptions {
    /// Keep at most this many messages; 0 means unlimited.
    pub max_messages: usize,
    /// URL prefix the web UI and API are served under, e.g. `/` or `/mail/`.
    pub webroot: String,
}

impl Default for AppOptions {
    fn default() -> Self {
        Self {
            max_messages: 0,
            webroot: "/".into(),
        }
    }
}

/// Cheap to clone; every clone shares the same store and event channel.
#[derive(Clone)]
pub struct Mailroom {
    inner: Arc<Inner>,
}

struct Inner {
    store: Store,
    events: broadcast::Sender<Event>,
    options: AppOptions,
    started: Instant,
}

impl Mailroom {
    pub fn new(store: Store, options: AppOptions) -> Self {
        let (events, _) = broadcast::channel(1024);
        Self {
            inner: Arc::new(Inner {
                store,
                events,
                options,
                started: Instant::now(),
            }),
        }
    }

    pub fn options(&self) -> &AppOptions {
        &self.inner.options
    }

    pub fn uptime(&self) -> std::time::Duration {
        self.inner.started.elapsed()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.inner.events.subscribe()
    }

    fn emit(&self, event: Event) {
        // An error only means nobody is listening.
        let _ = self.inner.events.send(event);
    }

    async fn emit_stats(&self) {
        if let Ok(stats) = self.stats().await {
            self.emit(Event::Stats(stats));
        }
    }

    /// Store a message received over SMTP.
    pub async fn ingest_smtp(
        &self,
        envelope: smtp::Envelope,
        peer: Option<SocketAddr>,
    ) -> Result<MessageSummary> {
        let stored = Envelope {
            source: "smtp".into(),
            mail_from: envelope.mail_from,
            rcpt_to: envelope.rcpt_to,
            helo: envelope.helo,
            remote_addr: peer.map(|p| p.to_string()),
            tls: envelope.tls,
            auth_user: envelope.auth_user,
        };
        self.ingest(stored, envelope.data).await
    }

    /// Compose and store a message submitted through the HTTP API.
    pub async fn ingest_draft(
        &self,
        draft: Draft,
    ) -> Result<std::result::Result<MessageSummary, String>> {
        let message_id = format!("{}@mailroom", new_id().to_ascii_lowercase());
        let raw = match compose::compose(&draft, &message_id, Utc::now()) {
            Ok(raw) => raw,
            Err(err) => return Ok(Err(err)),
        };
        let envelope = Envelope {
            source: "api".into(),
            mail_from: compose::bare_address(&draft.from).to_string(),
            rcpt_to: draft.recipients(),
            ..Default::default()
        };
        Ok(Ok(self.ingest(envelope, raw).await?))
    }

    /// Store a raw message with its envelope.
    pub async fn ingest(&self, envelope: Envelope, raw: Vec<u8>) -> Result<MessageSummary> {
        let id = new_id();
        let received_at = Utc::now();
        let keep = self.inner.options.max_messages;
        let (summary, pruned, stats) = self
            .inner
            .store
            .run(move |conn| {
                let analysis = parser::analyze(&raw, &envelope);
                let summary = repo::insert(
                    conn,
                    &repo::NewMessage {
                        id: &id,
                        received_at,
                        analysis: &analysis,
                        envelope: &envelope,
                        raw: &raw,
                    },
                )?;
                let pruned = if keep > 0 {
                    repo::prune(conn, keep)?
                } else {
                    Vec::new()
                };
                let stats = repo::stats(conn)?;
                Ok((summary, pruned, stats))
            })
            .await?;

        self.emit(Event::Created(Box::new(summary.clone())));
        if !pruned.is_empty() {
            self.emit(Event::Deleted { ids: pruned });
        }
        self.emit(Event::Stats(stats));
        Ok(summary)
    }

    pub async fn list(
        &self,
        query: SearchQuery,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<MessageSummary>, u64)> {
        self.inner
            .store
            .run(move |conn| repo::list(conn, &query, limit, offset))
            .await
    }

    pub async fn latest(&self, query: SearchQuery) -> Result<Option<MessageSummary>> {
        self.inner
            .store
            .run(move |conn| repo::latest(conn, &query))
            .await
    }

    pub async fn summary(&self, id: &str) -> Result<Option<MessageSummary>> {
        let id = id.to_string();
        self.inner
            .store
            .run(move |conn| repo::summary(conn, &id))
            .await
    }

    pub async fn raw(&self, id: &str) -> Result<Option<Vec<u8>>> {
        let id = id.to_string();
        self.inner.store.run(move |conn| repo::raw(conn, &id)).await
    }

    pub async fn detail(&self, id: &str) -> Result<Option<MessageDetail>> {
        let id = id.to_string();
        let base = format!("{}api/v1/messages/", self.inner.options.webroot);
        self.inner
            .store
            .run(move |conn| {
                let Some((summary, envelope, raw)) = repo::load(conn, &id)? else {
                    return Ok(None);
                };
                let rendered =
                    parser::render(&raw, |part| format!("{base}{}/parts/{part}", summary.id));
                Ok(Some(MessageDetail {
                    summary,
                    envelope,
                    headers: rendered.headers,
                    text: rendered.text,
                    html: rendered.html,
                    attachments: rendered.attachments,
                    inline: rendered.inline,
                    links: rendered.links,
                    codes: rendered.codes,
                }))
            })
            .await
    }

    pub async fn part(&self, id: &str, part_id: u32) -> Result<Option<PartContent>> {
        Ok(self
            .raw(id)
            .await?
            .and_then(|raw| parser::part(&raw, part_id)))
    }

    pub async fn set_read(&self, selection: Selection, read: bool) -> Result<Vec<String>> {
        let ids = self
            .inner
            .store
            .run(move |conn| repo::set_read(conn, &selection, read))
            .await?;
        if !ids.is_empty() {
            self.emit(Event::Updated {
                ids: ids.clone(),
                read,
            });
            self.emit_stats().await;
        }
        Ok(ids)
    }

    /// Delete the selected messages. Returns the removed ids, or `None` when
    /// every message was cleared.
    pub async fn delete(&self, selection: Selection) -> Result<Option<Vec<String>>> {
        let clear_all = matches!(selection, Selection::All);
        let ids = self
            .inner
            .store
            .run(move |conn| repo::delete(conn, &selection))
            .await?;
        if clear_all {
            self.emit(Event::Cleared);
            self.emit_stats().await;
            return Ok(None);
        }
        if !ids.is_empty() {
            self.emit(Event::Deleted { ids: ids.clone() });
            self.emit_stats().await;
        }
        Ok(Some(ids))
    }

    pub async fn stats(&self) -> Result<Stats> {
        self.inner.store.run(|conn| repo::stats(conn)).await
    }

    pub async fn recipients(&self, limit: usize) -> Result<Vec<Recipient>> {
        self.inner
            .store
            .run(move |conn| repo::recipients(conn, limit))
            .await
    }
}

/// A 26-character, time-sortable identifier (a UUIDv7 in Crockford base32).
pub fn new_id() -> String {
    const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let mut value = uuid::Uuid::now_v7().as_u128();
    let mut out = [0u8; 26];
    for slot in out.iter_mut().rev() {
        *slot = ALPHABET[(value & 31) as usize];
        value >>= 5;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_sortable() {
        let a = new_id();
        let b = new_id();
        assert_eq!(a.len(), 26);
        assert_ne!(a, b);
        assert!(a < b, "{a} should sort before {b}");
    }
}
