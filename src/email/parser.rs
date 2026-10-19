//! Turns raw RFC 5322 messages into Mailroom's internal model.
//!
//! MIME decoding is delegated to `mail-parser`; this module decides what a
//! message *means* for an inspection tool: who it was really sent to, which
//! parts are attachments versus inline images, and what to index for search.

use std::{
    collections::{HashMap, HashSet},
    sync::LazyLock,
};

use chrono::{DateTime, Utc};
use mail_parser::{
    Address as ParsedAddress, Message, MessageParser, MessagePart, MimeHeaders, PartType,
};
use regex::{Captures, Regex};

use super::{
    extract,
    model::{Address, Envelope, Header, Link, PartInfo},
};

const SNIPPET_CHARS: usize = 200;
/// Upper bound on the body text indexed for search, in bytes.
const SEARCH_TEXT_LIMIT: usize = 512 * 1024;

/// Summary fields and search index computed once, when a message arrives.
#[derive(Debug, Clone, Default)]
pub struct Analysis {
    pub message_id: Option<String>,
    pub subject: String,
    pub from: Option<Address>,
    pub to: Vec<Address>,
    pub cc: Vec<Address>,
    pub bcc: Vec<Address>,
    pub reply_to: Vec<Address>,
    pub date: Option<DateTime<Utc>>,
    pub snippet: String,
    pub attachment_count: u32,
    pub search: SearchIndex,
}

/// Lower-cased text that the search query language matches against.
#[derive(Debug, Clone, Default)]
pub struct SearchIndex {
    pub from: String,
    pub to: String,
    pub subject: String,
    pub text: String,
}

/// Body, headers and parts of a message, ready for display.
#[derive(Debug, Clone, Default)]
pub struct Rendered {
    pub headers: Vec<Header>,
    pub text: Option<String>,
    pub html: Option<String>,
    pub attachments: Vec<PartInfo>,
    pub inline: Vec<PartInfo>,
    pub links: Vec<Link>,
    pub codes: Vec<String>,
}

/// A single decoded MIME part, for downloading.
#[derive(Debug, Clone)]
pub struct PartContent {
    pub content_type: String,
    pub filename: Option<String>,
    pub data: Vec<u8>,
}

fn parse(raw: &[u8]) -> Option<Message<'_>> {
    MessageParser::default().parse(raw)
}

pub fn analyze(raw: &[u8], envelope: &Envelope) -> Analysis {
    let Some(message) = parse(raw) else {
        return analyze_unparseable(raw, envelope);
    };

    let from = message
        .from()
        .and_then(|a| addresses(Some(a)).into_iter().next());
    let to = addresses_all(message.all_to());
    let cc = addresses_all(message.all_cc());
    let mut bcc = addresses_all(message.all_bcc());
    let reply_to = addresses(message.reply_to());

    // Envelope recipients missing from every header were blind-copied.
    let mut seen: HashSet<String> = to
        .iter()
        .chain(&cc)
        .chain(&bcc)
        .map(|a| a.address.to_lowercase())
        .collect();
    for rcpt in &envelope.rcpt_to {
        if seen.insert(rcpt.to_lowercase()) {
            bcc.push(Address {
                name: None,
                address: rcpt.clone(),
            });
        }
    }

    let subject = message.subject().unwrap_or_default().trim().to_string();
    let body = body_text(&message);
    let classified = classify_parts(&message);

    let mut search_text = String::with_capacity(body.len() + 256);
    search_text.push_str(&subject);
    search_text.push('\n');
    for address in from.iter().chain(&to).chain(&cc).chain(&bcc) {
        push_address(&mut search_text, address);
    }
    search_text.push_str(&envelope.mail_from);
    search_text.push('\n');
    for &id in &classified.attachments {
        if let Some(name) = message.parts[id as usize].attachment_name() {
            search_text.push_str(name);
            search_text.push('\n');
        }
    }
    search_text.push_str(truncate(&body, SEARCH_TEXT_LIMIT));

    let mut search_from = String::new();
    if let Some(address) = &from {
        push_address(&mut search_from, address);
    }
    push_address(
        &mut search_from,
        &Address {
            name: None,
            address: envelope.mail_from.clone(),
        },
    );
    let mut search_to = String::new();
    for address in to.iter().chain(&cc).chain(&bcc) {
        push_address(&mut search_to, address);
    }

    Analysis {
        message_id: message.message_id().map(str::to_string),
        snippet: snippet(&body),
        attachment_count: classified.attachments.len() as u32,
        date: message
            .date()
            .and_then(|d| DateTime::from_timestamp(d.to_timestamp(), 0)),
        search: SearchIndex {
            from: search_from.to_lowercase(),
            to: search_to.to_lowercase(),
            subject: subject.to_lowercase(),
            text: search_text.to_lowercase(),
        },
        subject,
        from,
        to,
        cc,
        bcc,
        reply_to,
    }
}

fn analyze_unparseable(raw: &[u8], envelope: &Envelope) -> Analysis {
    let body = String::from_utf8_lossy(raw);
    let bcc: Vec<Address> = envelope
        .rcpt_to
        .iter()
        .map(|rcpt| Address {
            name: None,
            address: rcpt.clone(),
        })
        .collect();
    Analysis {
        snippet: snippet(&body),
        search: SearchIndex {
            from: envelope.mail_from.to_lowercase(),
            to: envelope.rcpt_to.join(" ").to_lowercase(),
            subject: String::new(),
            text: truncate(&body, SEARCH_TEXT_LIMIT).to_lowercase(),
        },
        bcc,
        ..Default::default()
    }
}

/// Render a message for display. `part_url` maps a part id to its download URL.
pub fn render(raw: &[u8], part_url: impl Fn(u32) -> String) -> Rendered {
    let Some(message) = parse(raw) else {
        return Rendered {
            text: Some(String::from_utf8_lossy(raw).into_owned()),
            ..Default::default()
        };
    };

    let headers = message
        .headers_raw()
        .map(|(name, value)| Header {
            name: name.to_string(),
            value: unfold(value),
        })
        .collect();

    let text = join_bodies(&message, &message.text_body, |part| match &part.body {
        PartType::Text(text) => Some(text.as_ref()),
        _ => None,
    });
    let mut html = join_bodies(&message, &message.html_body, |part| match &part.body {
        PartType::Html(html) => Some(html.as_ref()),
        _ => None,
    });

    let classified = classify_parts(&message);
    let info = |id: u32| part_info(&message, id, &part_url);
    let attachments = classified.attachments.iter().map(|&id| info(id)).collect();
    let inline = classified.inline.iter().map(|&id| info(id)).collect();

    if let Some(body) = &mut html
        && !classified.cids.is_empty()
    {
        *body = rewrite_cids(body, &classified.cids, &part_url);
    }

    let links = extract::links(text.as_deref(), html.as_deref());
    let codes = extract::codes(message.subject().unwrap_or_default(), &body_text(&message));

    Rendered {
        headers,
        text,
        html,
        attachments,
        inline,
        links,
        codes,
    }
}

/// Decode a single MIME part by id.
pub fn part(raw: &[u8], part_id: u32) -> Option<PartContent> {
    let message = parse(raw)?;
    let part = message.parts.get(part_id as usize)?;
    if matches!(part.body, PartType::Multipart(_)) {
        return None;
    }
    let mut content_type = content_type(part);
    if matches!(part.body, PartType::Text(_) | PartType::Html(_)) {
        // mail-parser has already decoded text parts to UTF-8.
        content_type.push_str("; charset=utf-8");
    }
    let filename = part
        .attachment_name()
        .map(str::to_string)
        .or_else(|| matches!(part.body, PartType::Message(_)).then(|| "message.eml".to_string()));
    Some(PartContent {
        content_type,
        filename,
        data: part.contents().to_vec(),
    })
}

struct Classified {
    attachments: Vec<u32>,
    inline: Vec<u32>,
    /// Content-ID (without angle brackets) to part id, for inline parts.
    cids: HashMap<String, u32>,
}

/// Split non-body parts into downloadable attachments and inline parts that
/// the HTML body references by Content-ID.
fn classify_parts(message: &Message<'_>) -> Classified {
    let html: String = message
        .html_body
        .iter()
        .filter_map(|&id| match &message.parts.get(id as usize)?.body {
            PartType::Html(html) => Some(html.as_ref()),
            _ => None,
        })
        .collect();

    let mut classified = Classified {
        attachments: Vec::new(),
        inline: Vec::new(),
        cids: HashMap::new(),
    };
    for &id in &message.attachments {
        let Some(part) = message.parts.get(id as usize) else {
            continue;
        };
        let explicit_attachment = part
            .content_disposition()
            .is_some_and(|d| d.ctype().eq_ignore_ascii_case("attachment"));
        let cid = part.content_id().map(clean_cid);
        match cid {
            Some(cid) if !explicit_attachment && html.contains(&format!("cid:{cid}")) => {
                classified.inline.push(id);
                classified.cids.insert(cid, id);
            }
            _ => classified.attachments.push(id),
        }
    }
    classified
}

fn part_info(message: &Message<'_>, id: u32, part_url: &impl Fn(u32) -> String) -> PartInfo {
    let part = &message.parts[id as usize];
    PartInfo {
        part_id: id,
        filename: part.attachment_name().map(str::to_string),
        content_type: content_type(part),
        size: part.len(),
        content_id: part.content_id().map(clean_cid),
        url: part_url(id),
    }
}

fn content_type(part: &MessagePart<'_>) -> String {
    if let Some(ct) = part.content_type() {
        let value = match ct.subtype() {
            Some(subtype) => format!("{}/{}", ct.ctype(), subtype),
            None => ct.ctype().to_string(),
        };
        return value.to_ascii_lowercase();
    }
    match part.body {
        PartType::Text(_) => "text/plain",
        PartType::Html(_) => "text/html",
        PartType::Message(_) => "message/rfc822",
        _ => "application/octet-stream",
    }
    .to_string()
}

fn clean_cid(cid: &str) -> String {
    cid.trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .to_string()
}

fn rewrite_cids(
    html: &str,
    cids: &HashMap<String, u32>,
    part_url: &impl Fn(u32) -> String,
) -> String {
    static CID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)cid:([^"'\s)>]+)"#).unwrap());
    CID.replace_all(html, |caps: &Captures<'_>| match cids.get(&caps[1]) {
        Some(&id) => part_url(id),
        None => caps[0].to_string(),
    })
    .into_owned()
}

/// Concatenate the body parts that `select` accepts, or `None` if there are none.
fn join_bodies<'a>(
    message: &'a Message<'a>,
    ids: &[u32],
    select: impl Fn(&'a MessagePart<'a>) -> Option<&'a str>,
) -> Option<String> {
    let bodies: Vec<&str> = ids
        .iter()
        .filter_map(|&id| message.parts.get(id as usize))
        .filter_map(select)
        .collect();
    (!bodies.is_empty()).then(|| bodies.join("\n\n"))
}

/// The readable text of a message: its text parts, or its HTML converted to
/// text when there is no plain-text alternative.
fn body_text(message: &Message<'_>) -> String {
    let mut out = String::new();
    for i in 0..message.text_body.len() {
        if let Some(text) = message.body_text(i) {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(&text);
        }
    }
    out
}

fn addresses(address: Option<&ParsedAddress<'_>>) -> Vec<Address> {
    address.map(|a| convert_addresses(a)).unwrap_or_default()
}

fn addresses_all<'a>(all: impl Iterator<Item = &'a ParsedAddress<'a>>) -> Vec<Address> {
    all.flat_map(convert_addresses).collect()
}

fn convert_addresses(address: &ParsedAddress<'_>) -> Vec<Address> {
    address
        .iter()
        .filter_map(|addr| {
            let email = addr.address()?.trim();
            if email.is_empty() {
                return None;
            }
            Some(Address {
                name: addr
                    .name()
                    .map(str::trim)
                    .filter(|name| !name.is_empty() && *name != email)
                    .map(str::to_string),
                address: email.to_string(),
            })
        })
        .collect()
}

fn push_address(out: &mut String, address: &Address) {
    if let Some(name) = &address.name {
        out.push_str(name);
        out.push(' ');
    }
    out.push_str(&address.address);
    out.push('\n');
}

fn snippet(body: &str) -> String {
    let mut out = String::with_capacity(SNIPPET_CHARS);
    let mut chars = 0;
    for word in body.split_whitespace() {
        if chars >= SNIPPET_CHARS {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
            chars += 1;
        }
        for c in word.chars() {
            if chars >= SNIPPET_CHARS {
                break;
            }
            out.push(c);
            chars += 1;
        }
    }
    out
}

fn truncate(text: &str, max_bytes: usize) -> &str {
    if text.len() <= max_bytes {
        return text;
    }
    let mut end = max_bytes;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Unfold a raw header value (RFC 5322 §2.2.3) for display.
fn unfold(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for line in value.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.trim().is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(line.trim());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope(rcpts: &[&str]) -> Envelope {
        Envelope {
            source: "smtp".into(),
            mail_from: "sender@example.com".into(),
            rcpt_to: rcpts.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    const MULTIPART: &str = concat!(
        "From: \"Alice Example\" <alice@example.com>\r\n",
        "To: Bob <bob@example.com>, carol@example.com\r\n",
        "Cc: dave@example.com\r\n",
        "Subject: =?UTF-8?B?V2VsY29tZSDwn46J?=\r\n",
        "Date: Mon, 19 Oct 2026 10:00:00 +0000\r\n",
        "Message-ID: <abc123@example.com>\r\n",
        "MIME-Version: 1.0\r\n",
        "Content-Type: multipart/mixed; boundary=\"outer\"\r\n",
        "\r\n",
        "--outer\r\n",
        "Content-Type: multipart/related; boundary=\"rel\"\r\n",
        "\r\n",
        "--rel\r\n",
        "Content-Type: multipart/alternative; boundary=\"alt\"\r\n",
        "\r\n",
        "--alt\r\n",
        "Content-Type: text/plain; charset=utf-8\r\n",
        "\r\n",
        "Hello Bob, your code is 482913.\r\n",
        "--alt\r\n",
        "Content-Type: text/html; charset=utf-8\r\n",
        "\r\n",
        "<p>Hello <b>Bob</b></p><img src=\"cid:logo@example.com\"><a href=\"https://example.com/verify?t=1&amp;u=2\">Verify</a>\r\n",
        "--alt--\r\n",
        "--rel\r\n",
        "Content-Type: image/png\r\n",
        "Content-ID: <logo@example.com>\r\n",
        "Content-Transfer-Encoding: base64\r\n",
        "\r\n",
        "iVBORw0KGgo=\r\n",
        "--rel--\r\n",
        "--outer\r\n",
        "Content-Type: application/pdf; name=\"invoice.pdf\"\r\n",
        "Content-Disposition: attachment; filename=\"invoice.pdf\"\r\n",
        "Content-Transfer-Encoding: base64\r\n",
        "\r\n",
        "JVBERi0xLjQK\r\n",
        "--outer--\r\n",
    );

    #[test]
    fn analyzes_headers_and_recipients() {
        let analysis = analyze(
            MULTIPART.as_bytes(),
            &envelope(&[
                "bob@example.com",
                "carol@example.com",
                "dave@example.com",
                "eve@example.com",
            ]),
        );
        assert_eq!(analysis.subject, "Welcome 🎉");
        assert_eq!(analysis.message_id.as_deref(), Some("abc123@example.com"));
        let from = analysis.from.unwrap();
        assert_eq!(from.name.as_deref(), Some("Alice Example"));
        assert_eq!(from.address, "alice@example.com");
        assert_eq!(analysis.to.len(), 2);
        assert_eq!(analysis.cc[0].address, "dave@example.com");
        // eve only appears in the envelope, so she was blind-copied.
        assert_eq!(analysis.bcc.len(), 1);
        assert_eq!(analysis.bcc[0].address, "eve@example.com");
        assert_eq!(analysis.attachment_count, 1);
        assert_eq!(analysis.snippet, "Hello Bob, your code is 482913.");
        assert!(analysis.date.is_some());
        assert!(analysis.search.text.contains("invoice.pdf"));
        assert!(analysis.search.to.contains("eve@example.com"));
    }

    #[test]
    fn renders_bodies_parts_and_extractions() {
        let rendered = render(MULTIPART.as_bytes(), |id| format!("/parts/{id}"));
        assert_eq!(
            rendered.text.as_deref(),
            Some("Hello Bob, your code is 482913.")
        );
        let html = rendered.html.unwrap();
        assert!(html.contains("src=\"/parts/"), "cid not rewritten: {html}");
        assert_eq!(rendered.inline.len(), 1);
        assert_eq!(rendered.inline[0].content_type, "image/png");
        assert_eq!(rendered.attachments.len(), 1);
        assert_eq!(
            rendered.attachments[0].filename.as_deref(),
            Some("invoice.pdf")
        );
        assert_eq!(rendered.links[0].url, "https://example.com/verify?t=1&u=2");
        assert_eq!(rendered.links[0].text.as_deref(), Some("Verify"));
        assert_eq!(rendered.codes, vec!["482913"]);
        assert!(rendered.headers.iter().any(|h| h.name == "Subject"));

        let pdf = part(MULTIPART.as_bytes(), rendered.attachments[0].part_id).unwrap();
        assert_eq!(pdf.content_type, "application/pdf");
        assert_eq!(pdf.data, b"%PDF-1.4\n");
    }

    #[test]
    fn html_only_message_has_no_text_part() {
        let raw = "Subject: Hi\r\nContent-Type: text/html\r\n\r\n<h1>Title</h1><p>Body text</p>";
        let rendered = render(raw.as_bytes(), |id| id.to_string());
        assert!(rendered.text.is_none());
        assert!(rendered.html.is_some());
        let analysis = analyze(raw.as_bytes(), &envelope(&[]));
        assert!(
            analysis.snippet.contains("Body text"),
            "{}",
            analysis.snippet
        );
    }

    #[test]
    fn unfolds_headers() {
        assert_eq!(unfold(" first\r\n second\r\n"), "first second");
    }

    #[test]
    fn snippet_collapses_whitespace() {
        assert_eq!(snippet("  a\n\n b\t c "), "a b c");
        assert_eq!(snippet(&"x".repeat(500)).chars().count(), SNIPPET_CHARS);
    }
}
