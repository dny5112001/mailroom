//! Message persistence in SQLite, either in memory (the default) or in a file.

pub mod query;
pub mod repo;

use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use rusqlite::Connection;

pub use query::SearchQuery;
pub use repo::{Recipient, Selection, Stats};

/// A shared SQLite connection. Queries run on Tokio's blocking thread pool so
/// they never stall the async runtime.
#[derive(Clone)]
pub struct Store {
    conn: Arc<Mutex<Connection>>,
}

impl Store {
    pub fn open(path: Option<&Path>) -> Result<Self> {
        let conn = match path {
            Some(path) => {
                if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                    std::fs::create_dir_all(parent)
                        .with_context(|| format!("creating {}", parent.display()))?;
                }
                let conn = Connection::open(path)
                    .with_context(|| format!("opening database {}", path.display()))?;
                conn.pragma_update(None, "journal_mode", "WAL")?;
                conn.pragma_update(None, "synchronous", "NORMAL")?;
                conn
            }
            None => Connection::open_in_memory()?,
        };
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        repo::migrate(&conn).context("migrating database schema")?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn in_memory() -> Result<Self> {
        Self::open(None)
    }

    /// Run `f` with exclusive access to the connection on a blocking thread.
    pub async fn run<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut Connection) -> rusqlite::Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let conn = self.conn.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut conn = conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            f(&mut conn)
        })
        .await
        .context("database task panicked")?;
        Ok(result?)
    }
}
