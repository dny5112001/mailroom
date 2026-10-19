//! Pull the things tests usually need out of an email: its links and any
//! one-time verification codes.

use std::{collections::HashSet, sync::LazyLock};

use linkify::{LinkFinder, LinkKind};
use regex::Regex;

use super::model::Link;

/// Links from the HTML body (with anchor text) followed by any additional
/// links found in the plain-text body, de-duplicated in order of appearance.
pub fn links(text: Option<&str>, html: Option<&str>) -> Vec<Link> {
    static ANCHOR: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r#"(?is)<a\b[^>]*?\bhref\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))[^>]*>(.*?)</a\s*>"#,
        )
        .unwrap()
    });
    static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<[^>]*>").unwrap());

    let mut seen = HashSet::new();
    let mut out = Vec::new();

    if let Some(html) = html {
        for caps in ANCHOR.captures_iter(html) {
            let href = caps
                .get(1)
                .or_else(|| caps.get(2))
                .or_else(|| caps.get(3))
                .map_or("", |m| m.as_str());
            let url = decode_entities(href.trim());
            if !is_followable(&url) || !seen.insert(url.clone()) {
                continue;
            }
            let label = decode_entities(&TAG.replace_all(&caps[4], " "));
            let label = label.split_whitespace().collect::<Vec<_>>().join(" ");
            out.push(Link {
                url,
                text: (!label.is_empty()).then_some(label),
            });
        }
    }

    if let Some(text) = text {
        let mut finder = LinkFinder::new();
        finder.kinds(&[LinkKind::Url]);
        for link in finder.links(text) {
            let url = link.as_str().to_string();
            if is_followable(&url) && seen.insert(url.clone()) {
                out.push(Link { url, text: None });
            }
        }
    }
    out
}

fn is_followable(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:")
}

/// Best-effort detection of one-time codes ("Your verification code is
/// 482913"). Candidates near words like "code" or "OTP" rank first; without
/// such context only six-digit numbers are reported.
pub fn codes(subject: &str, body: &str) -> Vec<String> {
    static KEYWORD: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)\b(code|otp|one[- ]time|passcode|password|pin|verification|verify|confirm(ation)?|security|token|login|sign[- ]in|2fa|mfa)\b",
        )
        .unwrap()
    });
    static CANDIDATE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\b(\d{3}[ -]\d{3}|\d{4,8}|[A-Z0-9]{6,10})\b").unwrap());

    let text = format!("{subject}\n{body}");
    let keyword_spans: Vec<(usize, usize)> = KEYWORD
        .find_iter(&text)
        .map(|m| (m.start(), m.end()))
        .collect();

    let mut scored: Vec<(i32, usize, String)> = Vec::new();
    for m in CANDIDATE.find_iter(&text) {
        let raw = m.as_str();
        if !looks_like_standalone_code(&text, m.start(), m.end()) {
            continue;
        }
        let code: String = raw.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        let digits = code.chars().filter(char::is_ascii_digit).count();
        let letters = code.len() - digits;
        // Alphanumeric candidates need a mix, or every capitalised word matches.
        if letters > 0 && (digits == 0 || letters == code.len()) {
            continue;
        }
        if letters == 0 && code.len() > 8 {
            continue; // phone numbers, order ids, timestamps
        }

        let near_keyword = keyword_spans.iter().any(|&(start, end)| {
            (m.start() >= end && m.start() - end <= 80)
                || (start >= m.end() && start - m.end() <= 30)
        });
        let mut score = 0;
        if near_keyword {
            score += 10;
        }
        if digits == code.len() && code.len() == 6 {
            score += 3;
        }
        if digits == 4 && letters == 0 && (code.starts_with("19") || code.starts_with("20")) {
            score -= 6; // probably a year
        }
        if near_keyword || (digits == 6 && letters == 0) {
            scored.push((score, m.start(), code));
        }
    }

    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut seen = HashSet::new();
    scored
        .into_iter()
        .filter(|(score, _, _)| *score > 0)
        .filter_map(|(_, _, code)| seen.insert(code.clone()).then_some(code))
        .take(5)
        .collect()
}

/// Reject numbers that are part of prices, dates, times, URLs or identifiers.
fn looks_like_standalone_code(text: &str, start: usize, end: usize) -> bool {
    let before = text[..start].chars().next_back();
    let after = text[end..].chars().next();
    let joined = |c: Option<char>| {
        matches!(
            c,
            Some('$' | '€' | '£' | '₹' | '#' | '/' | '=' | '&' | '?' | '%' | '@' | '-' | '_' | '+')
        )
    };
    let numeric_join = |c: Option<char>, next: Option<char>| {
        matches!(c, Some('.' | ',' | ':')) && next.is_some_and(|n| n.is_ascii_digit())
    };
    let after_next = text[end..].chars().nth(1);
    let before_prev = text[..start].chars().rev().nth(1);
    !(joined(before)
        || joined(after)
        || numeric_join(after, after_next)
        || numeric_join(before, before_prev))
}

fn decode_entities(input: &str) -> String {
    if !input.contains('&') {
        return input.to_string();
    }
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        rest = &rest[pos..];
        let Some(end) = rest[..rest.len().min(12)].find(';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|dec| dec.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_html_and_text_links() {
        let html = r##"<a class="btn" href="https://app.test/reset?token=a&amp;b=1"><span>Reset</span> password</a>
            <a href='mailto:help@app.test'>Help</a> <a href="#top">top</a> <a href="https://app.test/reset?token=a&amp;b=1">dup</a>"##;
        let text = "Or open https://app.test/other.";
        let found = links(Some(text), Some(html));
        assert_eq!(found.len(), 3);
        assert_eq!(found[0].url, "https://app.test/reset?token=a&b=1");
        assert_eq!(found[0].text.as_deref(), Some("Reset password"));
        assert_eq!(found[1].url, "mailto:help@app.test");
        assert_eq!(found[2].url, "https://app.test/other");
        assert_eq!(found[2].text, None);
    }

    #[test]
    fn finds_codes_near_keywords() {
        assert_eq!(
            codes("Your login code", "Use 482913 to sign in."),
            vec!["482913"]
        );
        assert_eq!(
            codes("Verify", "Your verification code is: 4821"),
            vec!["4821"]
        );
        assert_eq!(codes("", "Your code: 123 456"), vec!["123456"]);
        assert_eq!(codes("", "Enter code A7K2Q9 to continue"), vec!["A7K2Q9"]);
    }

    #[test]
    fn ignores_noise() {
        assert!(
            codes(
                "Invoice",
                "Total $1234 due 2026-10-19 at 10:30. Order #998877"
            )
            .is_empty()
        );
        assert!(codes("Newsletter", "Since 2019 we have shipped 1000 features").is_empty());
        assert!(codes("Hello", "Welcome to THE APP").is_empty());
    }

    #[test]
    fn six_digit_numbers_without_context_are_reported() {
        assert_eq!(codes("Welcome", "Here it is: 739201"), vec!["739201"]);
    }

    #[test]
    fn decodes_entities() {
        assert_eq!(
            decode_entities("a&amp;b&#39;c&#x41;&bogus;"),
            "a&b'cA&bogus;"
        );
    }
}
