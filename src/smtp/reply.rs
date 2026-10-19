use std::fmt;

/// An SMTP reply: a three-digit code and one or more text lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub code: u16,
    pub lines: Vec<String>,
}

impl Reply {
    pub fn new(code: u16, text: impl Into<String>) -> Self {
        Self {
            code,
            lines: vec![text.into()],
        }
    }

    pub fn multiline(code: u16, lines: Vec<String>) -> Self {
        debug_assert!(!lines.is_empty());
        Self { code, lines }
    }

    pub fn is_positive(&self) -> bool {
        self.code < 400
    }

    /// Serialize the reply in wire format (`250-first\r\n250 last\r\n`).
    pub fn to_wire(&self) -> String {
        let mut out = String::new();
        let last = self.lines.len().saturating_sub(1);
        for (i, line) in self.lines.iter().enumerate() {
            let sep = if i == last { ' ' } else { '-' };
            out.push_str(&format!("{}{}{}\r\n", self.code, sep, line));
        }
        out
    }
}

impl fmt::Display for Reply {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.code, self.lines.join(" / "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line_wire_format() {
        assert_eq!(Reply::new(250, "2.0.0 Ok").to_wire(), "250 2.0.0 Ok\r\n");
    }

    #[test]
    fn multiline_wire_format() {
        let reply = Reply::multiline(
            250,
            vec!["mailroom".into(), "SIZE 10".into(), "8BITMIME".into()],
        );
        assert_eq!(
            reply.to_wire(),
            "250-mailroom\r\n250-SIZE 10\r\n250 8BITMIME\r\n"
        );
    }
}
