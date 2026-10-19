use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Address {
    pub name: Option<String>,
    pub address: String,
}

/// How a message reached Mailroom, as opposed to what its headers claim.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Envelope {
    /// `smtp` or `api`.
    pub source: String,
    pub mail_from: String,
    pub rcpt_to: Vec<String>,
    pub helo: Option<String>,
    pub remote_addr: Option<String>,
    pub tls: bool,
    pub auth_user: Option<String>,
}

/// The fields shown in a message list.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MessageSummary {
    pub id: String,
    pub message_id: Option<String>,
    pub subject: String,
    pub from: Option<Address>,
    pub to: Vec<Address>,
    pub cc: Vec<Address>,
    /// Bcc header addresses plus envelope recipients that appear in no header.
    pub bcc: Vec<Address>,
    pub reply_to: Vec<Address>,
    pub snippet: String,
    pub size: u64,
    pub attachment_count: u32,
    pub read: bool,
    /// The message's own `Date` header.
    pub date: Option<DateTime<Utc>>,
    pub received_at: DateTime<Utc>,
}

/// A fully rendered message.
#[derive(Debug, Clone, Serialize)]
pub struct MessageDetail {
    #[serde(flatten)]
    pub summary: MessageSummary,
    pub envelope: Envelope,
    pub headers: Vec<Header>,
    /// Plain-text body, if the message has one.
    pub text: Option<String>,
    /// HTML body with `cid:` references rewritten to part URLs.
    pub html: Option<String>,
    pub attachments: Vec<PartInfo>,
    /// Parts referenced from the HTML body by Content-ID.
    pub inline: Vec<PartInfo>,
    pub links: Vec<Link>,
    pub codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Header {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PartInfo {
    pub part_id: u32,
    pub filename: Option<String>,
    pub content_type: String,
    pub size: usize,
    pub content_id: Option<String>,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Link {
    pub url: String,
    /// Anchor text, for links found in the HTML body.
    pub text: Option<String>,
}
