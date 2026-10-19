//! Build RFC 5322 messages from structured input (used by `POST /api/v1/send`).

use std::collections::BTreeMap;

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Draft {
    /// `"Name <address>"` or a bare address.
    pub from: String,
    #[serde(default)]
    pub to: Vec<String>,
    #[serde(default)]
    pub cc: Vec<String>,
    #[serde(default)]
    pub bcc: Vec<String>,
    #[serde(default)]
    pub reply_to: Option<String>,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub html: Option<String>,
    /// Extra headers, e.g. `{"X-Campaign": "welcome"}`.
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub attachments: Vec<DraftAttachment>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DraftAttachment {
    pub filename: String,
    #[serde(default = "default_attachment_type")]
    pub content_type: String,
    /// Base64-encoded file content.
    pub content: String,
}

fn default_attachment_type() -> String {
    "application/octet-stream".into()
}

impl Draft {
    /// Every envelope recipient: To, Cc and Bcc.
    pub fn recipients(&self) -> Vec<String> {
        self.to
            .iter()
            .chain(&self.cc)
            .chain(&self.bcc)
            .map(|r| bare_address(r).to_string())
            .collect()
    }
}

/// Render the draft as a MIME message. Bcc recipients are deliberately not
/// written to the headers, as a real mail client would do.
pub fn compose(draft: &Draft, message_id: &str, now: DateTime<Utc>) -> Result<Vec<u8>, String> {
    if draft.from.trim().is_empty() {
        return Err("`from` is required".into());
    }
    if draft.recipients().is_empty() {
        return Err("at least one recipient (to, cc or bcc) is required".into());
    }
    let mut headers: Vec<(String, String)> = vec![("From".into(), encode_address(&draft.from)?)];
    if !draft.to.is_empty() {
        headers.push(("To".into(), encode_address_list(&draft.to)?));
    }
    if !draft.cc.is_empty() {
        headers.push(("Cc".into(), encode_address_list(&draft.cc)?));
    }
    if let Some(reply_to) = &draft.reply_to {
        headers.push(("Reply-To".into(), encode_address(reply_to)?));
    }
    headers.push(("Subject".into(), encode_word(check_header(&draft.subject)?)));
    headers.push(("Date".into(), now.to_rfc2822()));
    headers.push(("Message-ID".into(), format!("<{message_id}>")));
    headers.push(("MIME-Version".into(), "1.0".into()));
    for (name, value) in &draft.headers {
        let valid_name =
            !name.is_empty() && name.bytes().all(|b| b.is_ascii_graphic() && b != b':');
        if !valid_name {
            return Err(format!("invalid header name {name:?}"));
        }
        headers.push((name.clone(), encode_word(check_header(value)?)));
    }

    let mut out = String::new();
    for (name, value) in headers {
        out.push_str(&format!("{name}: {value}\r\n"));
    }

    let alternative = body_alternative(draft, message_id);
    if draft.attachments.is_empty() {
        out.push_str(&alternative);
        return Ok(out.into_bytes());
    }

    let boundary = format!("mixed-{message_id}");
    out.push_str(&format!(
        "Content-Type: multipart/mixed; boundary=\"{boundary}\"\r\n\r\n"
    ));
    out.push_str(&format!("--{boundary}\r\n{alternative}\r\n"));
    for attachment in &draft.attachments {
        let data = BASE64
            .decode(attachment.content.trim())
            .map_err(|_| format!("attachment {:?} is not valid base64", attachment.filename))?;
        let filename = check_header(&attachment.filename)?.replace('"', "'");
        let content_type = check_header(&attachment.content_type)?;
        out.push_str(&format!(
            "--{boundary}\r\nContent-Type: {content_type}; name=\"{filename}\"\r\n\
             Content-Disposition: attachment; filename=\"{filename}\"\r\n\
             Content-Transfer-Encoding: base64\r\n\r\n{}\r\n",
            wrap_base64(&data)
        ));
    }
    out.push_str(&format!("--{boundary}--\r\n"));
    Ok(out.into_bytes())
}

/// The body as a single part or a multipart/alternative, starting with its
/// Content-Type header.
fn body_alternative(draft: &Draft, message_id: &str) -> String {
    let text_part = |text: &str| {
        format!(
            "Content-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: base64\r\n\r\n{}\r\n",
            wrap_base64(text.as_bytes())
        )
    };
    let html_part = |html: &str| {
        format!(
            "Content-Type: text/html; charset=utf-8\r\nContent-Transfer-Encoding: base64\r\n\r\n{}\r\n",
            wrap_base64(html.as_bytes())
        )
    };
    match (&draft.text, &draft.html) {
        (Some(text), Some(html)) => {
            let boundary = format!("alt-{message_id}");
            format!(
                "Content-Type: multipart/alternative; boundary=\"{boundary}\"\r\n\r\n\
                 --{boundary}\r\n{}--{boundary}\r\n{}--{boundary}--\r\n",
                text_part(text),
                html_part(html)
            )
        }
        (None, Some(html)) => html_part(html),
        (Some(text), None) => text_part(text),
        (None, None) => text_part(""),
    }
}

/// `"Alice <alice@example.com>"` → `"alice@example.com"`.
pub fn bare_address(input: &str) -> &str {
    let input = input.trim();
    match (input.rfind('<'), input.rfind('>')) {
        (Some(start), Some(end)) if start < end => input[start + 1..end].trim(),
        _ => input,
    }
}

fn check_header(value: &str) -> Result<&str, String> {
    if value.contains(['\r', '\n']) {
        Err(format!(
            "header value {value:?} must not contain line breaks"
        ))
    } else {
        Ok(value)
    }
}

fn encode_address_list(list: &[String]) -> Result<String, String> {
    let encoded: Result<Vec<_>, _> = list.iter().map(|a| encode_address(a)).collect();
    Ok(encoded?.join(", "))
}

fn encode_address(input: &str) -> Result<String, String> {
    let input = check_header(input)?.trim();
    let address = bare_address(input);
    if address.is_empty() || address.contains(char::is_whitespace) {
        return Err(format!("invalid address {input:?}"));
    }
    let name = match input.rfind('<') {
        Some(start) => input[..start].trim().trim_matches('"').trim(),
        None => "",
    };
    if name.is_empty() {
        return Ok(address.to_string());
    }
    let name = if name.is_ascii() {
        format!("\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        encode_word(name)
    };
    Ok(format!("{name} <{address}>"))
}

/// RFC 2047 B-encoding for non-ASCII header text, split into encoded words of
/// at most 75 characters.
fn encode_word(text: &str) -> String {
    if text.is_ascii() {
        return text.to_string();
    }
    let mut words = Vec::new();
    let mut chunk = String::new();
    for c in text.chars() {
        if chunk.len() + c.len_utf8() > 45 {
            words.push(format!("=?UTF-8?B?{}?=", BASE64.encode(&chunk)));
            chunk.clear();
        }
        chunk.push(c);
    }
    if !chunk.is_empty() {
        words.push(format!("=?UTF-8?B?{}?=", BASE64.encode(&chunk)));
    }
    words.join("\r\n ")
}

fn wrap_base64(data: &[u8]) -> String {
    let encoded = BASE64.encode(data);
    let mut out = String::with_capacity(encoded.len() + encoded.len() / 76 * 2);
    for (i, chunk) in encoded.as_bytes().chunks(76).enumerate() {
        if i > 0 {
            out.push_str("\r\n");
        }
        out.push_str(std::str::from_utf8(chunk).unwrap_or_default());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::email::{model::Envelope, parser};

    #[test]
    fn composes_parseable_message() {
        let draft = Draft {
            from: "Mailroom Demo <demo@mailroom.test>".into(),
            to: vec!["Bób <bob@example.com>".into()],
            bcc: vec!["audit@example.com".into()],
            subject: "Héllo there".into(),
            text: Some("Plain body".into()),
            html: Some("<p>HTML body</p>".into()),
            attachments: vec![DraftAttachment {
                filename: "notes.txt".into(),
                content_type: "text/plain".into(),
                content: BASE64.encode("attached"),
            }],
            ..Default::default()
        };
        let raw = compose(&draft, "id1@mailroom", Utc::now()).unwrap();
        let envelope = Envelope {
            rcpt_to: draft.recipients(),
            ..Default::default()
        };
        let analysis = parser::analyze(&raw, &envelope);
        assert_eq!(analysis.subject, "Héllo there");
        assert_eq!(analysis.to[0].name.as_deref(), Some("Bób"));
        assert_eq!(analysis.bcc[0].address, "audit@example.com");
        assert_eq!(analysis.attachment_count, 1);

        let rendered = parser::render(&raw, |id| id.to_string());
        assert_eq!(rendered.text.as_deref(), Some("Plain body"));
        assert_eq!(rendered.html.as_deref(), Some("<p>HTML body</p>"));
        assert!(!String::from_utf8_lossy(&raw).contains("audit@example.com"));
    }

    #[test]
    fn rejects_header_injection() {
        let draft = Draft {
            from: "a@b.c".into(),
            to: vec!["d@e.f".into()],
            subject: "hi\r\nBcc: victim@example.com".into(),
            ..Default::default()
        };
        assert!(compose(&draft, "x", Utc::now()).is_err());
    }

    #[test]
    fn requires_recipients() {
        let draft = Draft {
            from: "a@b.c".into(),
            ..Default::default()
        };
        assert!(compose(&draft, "x", Utc::now()).is_err());
    }
}
