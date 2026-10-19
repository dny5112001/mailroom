//! The search language used by the inbox and the API.
//!
//! ```text
//! reset password            words anywhere (subject, addresses, body, filenames)
//! "reset password"          an exact phrase
//! from:alice  to:bob        sender / any recipient (To, Cc, Bcc, envelope)
//! subject:welcome           subject only
//! message-id:abc@host       Message-ID header
//! is:unread  is:read        read state
//! has:attachment            at least one attachment
//! after:2026-10-01          received on or after a date (or RFC 3339 time)
//! before:2026-10-19         received before a date
//! -newsletter  -from:noreply   negate any term
//! ```
//!
//! Matching is case-insensitive substring matching, so `to:@example.com`
//! finds every recipient at that domain.

use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::types::Value;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SearchQuery {
    pub conditions: Vec<Condition>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    pub negated: bool,
    pub kind: Kind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Text {
        field: Field,
        value: String,
    },
    Read(bool),
    HasAttachments,
    /// Received at or after this unix time in milliseconds.
    After(i64),
    /// Received before this unix time in milliseconds.
    Before(i64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Any,
    From,
    To,
    Subject,
    MessageId,
}

impl SearchQuery {
    pub fn parse(input: &str) -> Self {
        let conditions = tokenize(input)
            .into_iter()
            .filter_map(|token| parse_token(&token))
            .collect();
        Self { conditions }
    }

    pub fn is_empty(&self) -> bool {
        self.conditions.is_empty()
    }

    /// Restrict the query to messages received at or after `millis`.
    pub fn received_since(mut self, millis: i64) -> Self {
        self.conditions.push(Condition {
            negated: false,
            kind: Kind::After(millis),
        });
        self
    }

    /// Translate the query into a SQL `WHERE` clause and its parameters.
    pub fn to_sql(&self) -> (String, Vec<Value>) {
        if self.conditions.is_empty() {
            return ("1 = 1".into(), Vec::new());
        }
        let mut params = Vec::with_capacity(self.conditions.len());
        let clauses: Vec<String> = self
            .conditions
            .iter()
            .map(|condition| {
                let clause = match &condition.kind {
                    Kind::Text { field, value } => {
                        params.push(Value::Text(value.clone()));
                        let column = match field {
                            Field::Any => "search_text",
                            Field::From => "search_from",
                            Field::To => "search_to",
                            Field::Subject => "search_subject",
                            Field::MessageId => "lower(coalesce(message_id, ''))",
                        };
                        format!("instr({column}, ?) > 0")
                    }
                    Kind::Read(read) => {
                        params.push(Value::Integer(i64::from(*read)));
                        "read = ?".into()
                    }
                    Kind::HasAttachments => "attachment_count > 0".into(),
                    Kind::After(millis) => {
                        params.push(Value::Integer(*millis));
                        "received_at >= ?".into()
                    }
                    Kind::Before(millis) => {
                        params.push(Value::Integer(*millis));
                        "received_at < ?".into()
                    }
                };
                if condition.negated {
                    format!("NOT ({clause})")
                } else {
                    clause
                }
            })
            .collect();
        (clauses.join(" AND "), params)
    }
}

/// Split on whitespace, keeping double-quoted sections together. Quotes may
/// open mid-token, as in `subject:"hello world"`.
fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quoted = false;
    for c in input.chars() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                quoted = true;
            }
            c if c.is_whitespace() && !in_quotes => {
                if !current.is_empty() || quoted {
                    tokens.push(std::mem::take(&mut current));
                }
                quoted = false;
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn parse_token(token: &str) -> Option<Condition> {
    let (negated, body) = match token.strip_prefix('-') {
        Some(rest) if !rest.is_empty() => (true, rest),
        _ => (false, token),
    };
    if body.is_empty() {
        return None;
    }
    let kind = parse_keyword(body).unwrap_or_else(|| Kind::Text {
        field: Field::Any,
        value: body.to_lowercase(),
    });
    Some(Condition { negated, kind })
}

fn parse_keyword(body: &str) -> Option<Kind> {
    let (key, value) = body.split_once(':')?;
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let text = |field| {
        Some(Kind::Text {
            field,
            value: value.to_lowercase(),
        })
    };
    match key.to_ascii_lowercase().as_str() {
        "from" => text(Field::From),
        "to" | "cc" | "bcc" | "rcpt" => text(Field::To),
        "subject" => text(Field::Subject),
        "message-id" | "messageid" | "mid" => text(Field::MessageId),
        "is" => match value.to_ascii_lowercase().as_str() {
            "read" | "seen" => Some(Kind::Read(true)),
            "unread" | "unseen" | "new" => Some(Kind::Read(false)),
            _ => None,
        },
        "has" => matches!(
            value.to_ascii_lowercase().as_str(),
            "attachment" | "attachments" | "file" | "files"
        )
        .then_some(Kind::HasAttachments),
        "after" | "since" => parse_time(value).map(Kind::After),
        "before" | "until" => parse_time(value).map(Kind::Before),
        _ => None,
    }
}

/// Parse a date (`2026-10-19`, midnight UTC), an RFC 3339 timestamp, or unix
/// milliseconds.
pub fn parse_time(value: &str) -> Option<i64> {
    if let Ok(time) = DateTime::parse_from_rfc3339(value) {
        return Some(time.timestamp_millis());
    }
    if let Ok(date) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        return Some(date.and_hms_opt(0, 0, 0)?.and_utc().timestamp_millis());
    }
    let number: i64 = value.parse().ok()?;
    // Accept seconds as well as milliseconds.
    Some(if number < 100_000_000_000 {
        number * 1000
    } else {
        number
    })
}

pub fn now_millis() -> i64 {
    Utc::now().timestamp_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(field: Field, value: &str, negated: bool) -> Condition {
        Condition {
            negated,
            kind: Kind::Text {
                field,
                value: value.into(),
            },
        }
    }

    #[test]
    fn parses_keywords_phrases_and_negation() {
        let query = SearchQuery::parse(
            r#"From:Alice subject:"Reset Password" -newsletter is:unread has:attachment hello"#,
        );
        assert_eq!(
            query.conditions,
            vec![
                text(Field::From, "alice", false),
                text(Field::Subject, "reset password", false),
                text(Field::Any, "newsletter", true),
                Condition {
                    negated: false,
                    kind: Kind::Read(false)
                },
                Condition {
                    negated: false,
                    kind: Kind::HasAttachments
                },
                text(Field::Any, "hello", false),
            ]
        );
    }

    #[test]
    fn unknown_keywords_are_plain_text() {
        let query = SearchQuery::parse("https://example.com is:bogus");
        assert_eq!(
            query.conditions,
            vec![
                text(Field::Any, "https://example.com", false),
                text(Field::Any, "is:bogus", false),
            ]
        );
    }

    #[test]
    fn parses_dates() {
        let query = SearchQuery::parse("after:2026-10-19 before:1700000000");
        assert_eq!(query.conditions[0].kind, Kind::After(1_792_368_000_000));
        assert_eq!(query.conditions[1].kind, Kind::Before(1_700_000_000_000));
    }

    #[test]
    fn builds_sql() {
        let (sql, params) = SearchQuery::parse("to:bob -is:read").to_sql();
        assert_eq!(sql, "instr(search_to, ?) > 0 AND NOT (read = ?)");
        assert_eq!(params, vec![Value::Text("bob".into()), Value::Integer(1)]);
        assert_eq!(SearchQuery::parse("  ").to_sql().0, "1 = 1");
    }
}
