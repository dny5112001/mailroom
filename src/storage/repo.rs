//! SQL for storing and querying messages. Every function takes a borrowed
//! connection and is synchronous; [`super::Store`] runs them off the async
//! runtime.

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, Row, params, params_from_iter, types::Value};
use serde::{Serialize, de::DeserializeOwned};

use super::query::SearchQuery;
use crate::email::{Envelope, MessageSummary, parser::Analysis};

const SCHEMA_VERSION: i64 = 1;

const SUMMARY_COLUMNS: &str = "id, message_id, subject, from_json, to_json, cc_json, bcc_json, \
     reply_to_json, snippet, size, attachment_count, read, date, received_at";

pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version >= SCHEMA_VERSION {
        return Ok(());
    }
    conn.execute_batch(
        "BEGIN;
         CREATE TABLE IF NOT EXISTS messages (
             seq              INTEGER PRIMARY KEY,
             id               TEXT    NOT NULL UNIQUE,
             received_at      INTEGER NOT NULL,
             date             INTEGER,
             message_id       TEXT,
             subject          TEXT    NOT NULL,
             from_json        TEXT,
             to_json          TEXT    NOT NULL,
             cc_json          TEXT    NOT NULL,
             bcc_json         TEXT    NOT NULL,
             reply_to_json    TEXT    NOT NULL,
             snippet          TEXT    NOT NULL,
             size             INTEGER NOT NULL,
             attachment_count INTEGER NOT NULL,
             read             INTEGER NOT NULL DEFAULT 0,
             envelope_json    TEXT    NOT NULL,
             search_from      TEXT    NOT NULL,
             search_to        TEXT    NOT NULL,
             search_subject   TEXT    NOT NULL,
             search_text      TEXT    NOT NULL
         );
         CREATE INDEX IF NOT EXISTS messages_received_at ON messages (received_at);
         CREATE TABLE IF NOT EXISTS message_raw (
             message_seq INTEGER PRIMARY KEY REFERENCES messages (seq) ON DELETE CASCADE,
             raw         BLOB    NOT NULL
         );
         PRAGMA user_version = 1;
         COMMIT;",
    )
}

/// Which messages an update or delete applies to.
#[derive(Debug, Clone)]
pub enum Selection {
    All,
    Ids(Vec<String>),
    Query(SearchQuery),
}

impl Selection {
    fn to_sql(&self) -> (String, Vec<Value>) {
        match self {
            Selection::All => ("1 = 1".into(), Vec::new()),
            Selection::Ids(ids) => (
                "id IN (SELECT value FROM json_each(?))".into(),
                vec![Value::Text(serde_json::to_string(ids).unwrap_or_default())],
            ),
            Selection::Query(query) => query.to_sql(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Stats {
    pub total: u64,
    pub unread: u64,
    pub size: u64,
}

pub struct NewMessage<'a> {
    pub id: &'a str,
    pub received_at: DateTime<Utc>,
    pub analysis: &'a Analysis,
    pub envelope: &'a Envelope,
    pub raw: &'a [u8],
}

pub fn insert(conn: &mut Connection, message: &NewMessage<'_>) -> rusqlite::Result<MessageSummary> {
    let a = message.analysis;
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO messages (id, received_at, date, message_id, subject, from_json, to_json,
             cc_json, bcc_json, reply_to_json, snippet, size, attachment_count, read,
             envelope_json, search_from, search_to, search_subject, search_text)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 0, ?14, ?15, ?16, ?17, ?18)",
        params![
            message.id,
            message.received_at.timestamp_millis(),
            a.date.map(|d| d.timestamp_millis()),
            a.message_id,
            a.subject,
            a.from.as_ref().map(to_json),
            to_json(&a.to),
            to_json(&a.cc),
            to_json(&a.bcc),
            to_json(&a.reply_to),
            a.snippet,
            message.raw.len() as i64,
            i64::from(a.attachment_count),
            to_json(message.envelope),
            a.search.from,
            a.search.to,
            a.search.subject,
            a.search.text,
        ],
    )?;
    let seq = tx.last_insert_rowid();
    tx.execute(
        "INSERT INTO message_raw (message_seq, raw) VALUES (?1, ?2)",
        params![seq, message.raw],
    )?;
    tx.commit()?;

    Ok(MessageSummary {
        id: message.id.to_string(),
        message_id: a.message_id.clone(),
        subject: a.subject.clone(),
        from: a.from.clone(),
        to: a.to.clone(),
        cc: a.cc.clone(),
        bcc: a.bcc.clone(),
        reply_to: a.reply_to.clone(),
        snippet: a.snippet.clone(),
        size: message.raw.len() as u64,
        attachment_count: a.attachment_count,
        read: false,
        date: a.date,
        received_at: message.received_at,
    })
}

/// A page of matching messages, newest first, and the total number of matches.
pub fn list(
    conn: &Connection,
    query: &SearchQuery,
    limit: usize,
    offset: usize,
) -> rusqlite::Result<(Vec<MessageSummary>, u64)> {
    let (clause, mut values) = query.to_sql();
    let matched: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM messages WHERE {clause}"),
        params_from_iter(values.iter()),
        |row| row.get(0),
    )?;
    values.push(Value::Integer(limit as i64));
    values.push(Value::Integer(offset as i64));
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {SUMMARY_COLUMNS} FROM messages WHERE {clause} ORDER BY seq DESC LIMIT ? OFFSET ?"
    ))?;
    let messages = stmt
        .query_map(params_from_iter(values.iter()), row_to_summary)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok((messages, matched as u64))
}

pub fn latest(conn: &Connection, query: &SearchQuery) -> rusqlite::Result<Option<MessageSummary>> {
    let (clause, values) = query.to_sql();
    conn.query_row(
        &format!("SELECT {SUMMARY_COLUMNS} FROM messages WHERE {clause} ORDER BY seq DESC LIMIT 1"),
        params_from_iter(values.iter()),
        row_to_summary,
    )
    .optional()
}

pub fn summary(conn: &Connection, id: &str) -> rusqlite::Result<Option<MessageSummary>> {
    conn.query_row(
        &format!("SELECT {SUMMARY_COLUMNS} FROM messages WHERE id = ?1"),
        [id],
        row_to_summary,
    )
    .optional()
}

/// Summary, envelope and raw source of one message.
pub fn load(
    conn: &Connection,
    id: &str,
) -> rusqlite::Result<Option<(MessageSummary, Envelope, Vec<u8>)>> {
    conn.query_row(
        &format!(
            "SELECT {SUMMARY_COLUMNS}, envelope_json, raw FROM messages
             JOIN message_raw ON message_raw.message_seq = messages.seq WHERE id = ?1"
        ),
        [id],
        |row| {
            let summary = row_to_summary(row)?;
            let envelope: Envelope = json_column(row, 14)?;
            let raw: Vec<u8> = row.get(15)?;
            Ok((summary, envelope, raw))
        },
    )
    .optional()
}

pub fn raw(conn: &Connection, id: &str) -> rusqlite::Result<Option<Vec<u8>>> {
    conn.query_row(
        "SELECT raw FROM message_raw JOIN messages ON message_raw.message_seq = messages.seq
         WHERE messages.id = ?1",
        [id],
        |row| row.get(0),
    )
    .optional()
}

/// Mark messages read or unread, returning the ids whose state changed.
pub fn set_read(
    conn: &Connection,
    selection: &Selection,
    read: bool,
) -> rusqlite::Result<Vec<String>> {
    let (clause, mut values) = selection.to_sql();
    values.insert(0, Value::Integer(i64::from(read)));
    values.push(Value::Integer(i64::from(read)));
    let mut stmt = conn.prepare(&format!(
        "UPDATE messages SET read = ? WHERE ({clause}) AND read != ? RETURNING id"
    ))?;
    stmt.query_map(params_from_iter(values.iter()), |row| row.get(0))?
        .collect()
}

/// Delete messages, returning the ids that were removed.
pub fn delete(conn: &Connection, selection: &Selection) -> rusqlite::Result<Vec<String>> {
    if matches!(selection, Selection::All) {
        conn.execute_batch("DELETE FROM message_raw; DELETE FROM messages;")?;
        return Ok(Vec::new());
    }
    let (clause, values) = selection.to_sql();
    let mut stmt = conn.prepare(&format!("DELETE FROM messages WHERE {clause} RETURNING id"))?;
    stmt.query_map(params_from_iter(values.iter()), |row| row.get(0))?
        .collect()
}

/// Delete all but the newest `keep` messages, returning the removed ids.
pub fn prune(conn: &Connection, keep: usize) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare_cached(
        "DELETE FROM messages
         WHERE seq <= (SELECT seq FROM messages ORDER BY seq DESC LIMIT 1 OFFSET ?1)
         RETURNING id",
    )?;
    stmt.query_map([keep as i64], |row| row.get(0))?.collect()
}

pub fn stats(conn: &Connection) -> rusqlite::Result<Stats> {
    conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(read = 0), 0), COALESCE(SUM(size), 0) FROM messages",
        [],
        |row| {
            Ok(Stats {
                total: row.get::<_, i64>(0)? as u64,
                unread: row.get::<_, i64>(1)? as u64,
                size: row.get::<_, i64>(2)? as u64,
            })
        },
    )
}

fn row_to_summary(row: &Row<'_>) -> rusqlite::Result<MessageSummary> {
    Ok(MessageSummary {
        id: row.get(0)?,
        message_id: row.get(1)?,
        subject: row.get(2)?,
        from: json_column(row, 3)?,
        to: json_column(row, 4)?,
        cc: json_column(row, 5)?,
        bcc: json_column(row, 6)?,
        reply_to: json_column(row, 7)?,
        snippet: row.get(8)?,
        size: row.get::<_, i64>(9)? as u64,
        attachment_count: row.get::<_, i64>(10)? as u32,
        read: row.get(11)?,
        date: row
            .get::<_, Option<i64>>(12)?
            .and_then(DateTime::from_timestamp_millis),
        received_at: DateTime::from_timestamp_millis(row.get(13)?).unwrap_or_default(),
    })
}

fn to_json<T: Serialize + ?Sized>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".into())
}

fn json_column<T: DeserializeOwned + Default>(row: &Row<'_>, index: usize) -> rusqlite::Result<T> {
    let text: Option<String> = row.get(index)?;
    Ok(text
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default())
}
