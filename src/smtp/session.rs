//! A sans-IO SMTP session state machine.
//!
//! [`Session`] consumes command lines and message content and tells the caller
//! what to do next through [`Action`]. It never touches a socket, which keeps
//! every protocol decision deterministic and unit-testable. The network loop in
//! [`super::server`] is a thin adapter around it.

use std::sync::Arc;

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};

use super::reply::Reply;

/// After this many failed commands the connection is closed.
const MAX_ERRORS: usize = 20;

#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Name announced in the greeting and EHLO response.
    pub hostname: String,
    /// Largest accepted message, in bytes.
    pub max_message_size: usize,
    /// Largest number of RCPT TO commands per transaction.
    pub max_recipients: usize,
    pub auth: AuthPolicy,
    /// Whether STARTTLS can be offered.
    pub tls_available: bool,
    /// Refuse MAIL and AUTH until the connection is encrypted.
    pub require_tls: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            hostname: "mailroom".into(),
            max_message_size: 25 * 1024 * 1024,
            max_recipients: 1000,
            auth: AuthPolicy::AcceptAny,
            tls_available: false,
            require_tls: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthPolicy {
    /// AUTH is advertised and any credentials succeed, but it is optional.
    AcceptAny,
    /// Clients must authenticate with these credentials before sending mail.
    Required { username: String, password: String },
}

/// What the network layer should do after feeding the session a line.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Reply(Reply),
    /// Send the reply, then read message content up to the `.` terminator and
    /// pass it to [`Session::handle_data`].
    ReadData(Reply),
    /// Read exactly `size` bytes of BDAT chunk data and pass them to
    /// [`Session::handle_chunk`]. Nothing is sent before the chunk is read.
    ReadChunk {
        size: usize,
        last: bool,
    },
    /// Send the reply, perform the TLS handshake, then call
    /// [`Session::tls_established`].
    StartTls(Reply),
    /// Send the reply and close the connection.
    Close(Reply),
}

/// Outcome of a BDAT chunk.
#[derive(Debug, PartialEq, Eq)]
pub enum ChunkOutcome {
    Reply(Reply),
    Deliver(Envelope),
}

/// A complete message accepted from the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    pub helo: Option<String>,
    pub mail_from: String,
    pub rcpt_to: Vec<String>,
    pub auth_user: Option<String>,
    pub tls: bool,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    Command,
    AuthPlain,
    AuthLoginUsername,
    AuthLoginPassword { username: String },
}

pub struct Session {
    config: Arc<SessionConfig>,
    state: State,
    helo: Option<String>,
    tls: bool,
    auth_user: Option<String>,
    mail_from: Option<String>,
    rcpt_to: Vec<String>,
    /// Accumulated BDAT content while a chunked transfer is in progress.
    chunks: Option<Vec<u8>>,
    /// Reply for a BDAT command that was rejected; its chunk is still read.
    chunk_error: Option<Reply>,
    errors: usize,
}

impl Session {
    pub fn new(config: Arc<SessionConfig>) -> Self {
        Self {
            config,
            state: State::Command,
            helo: None,
            tls: false,
            auth_user: None,
            mail_from: None,
            rcpt_to: Vec::new(),
            chunks: None,
            chunk_error: None,
            errors: 0,
        }
    }

    pub fn greeting(&self) -> Reply {
        Reply::new(
            220,
            format!("{} ESMTP Mailroom ready", self.config.hostname),
        )
    }

    pub fn timeout_reply(&self) -> Reply {
        Reply::new(
            421,
            format!("4.4.2 {} Error: timeout exceeded", self.config.hostname),
        )
    }

    /// Feed one line received from the client (with or without its CRLF).
    pub fn handle_line(&mut self, line: &[u8]) -> Action {
        let line = trim_line_ending(line);
        let action = match std::mem::replace(&mut self.state, State::Command) {
            State::Command => self.handle_command(line),
            State::AuthPlain => self.finish_auth_plain(line),
            State::AuthLoginUsername => self.auth_login_username(line),
            State::AuthLoginPassword { username } => self.auth_login_password(username, line),
        };
        self.track_errors(action)
    }

    /// Called when a command line exceeded the maximum length.
    pub fn line_too_long(&mut self) -> Action {
        self.state = State::Command;
        self.track_errors(Action::Reply(Reply::new(500, "5.5.6 Error: line too long")))
    }

    /// Finish a DATA transfer. `too_large` is set when the content exceeded the
    /// size limit and was discarded while reading.
    pub fn handle_data(&mut self, data: Vec<u8>, too_large: bool) -> Result<Envelope, Reply> {
        if too_large {
            self.reset_transaction();
            return Err(size_exceeded());
        }
        Ok(self.take_envelope(data))
    }

    /// Bytes that can still be accepted in the current BDAT transfer.
    pub fn chunk_capacity(&self) -> usize {
        let used = self.chunks.as_ref().map_or(0, Vec::len);
        self.config.max_message_size.saturating_sub(used)
    }

    /// Finish reading a BDAT chunk. `overflow` is set when the chunk did not fit
    /// in [`Self::chunk_capacity`] and was discarded.
    pub fn handle_chunk(&mut self, chunk: Vec<u8>, last: bool, overflow: bool) -> ChunkOutcome {
        if let Some(reply) = self.chunk_error.take() {
            return ChunkOutcome::Reply(reply);
        }
        if overflow {
            self.reset_transaction();
            return ChunkOutcome::Reply(size_exceeded());
        }
        let size = chunk.len();
        let buffer = self.chunks.get_or_insert_with(Vec::new);
        buffer.extend_from_slice(&chunk);
        if last {
            let data = self.chunks.take().unwrap_or_default();
            ChunkOutcome::Deliver(self.take_envelope(data))
        } else {
            ChunkOutcome::Reply(Reply::new(250, format!("2.0.0 {size} octets received")))
        }
    }

    /// Reset the session after a successful STARTTLS handshake (RFC 3207 §4.2).
    pub fn tls_established(&mut self) {
        self.tls = true;
        self.helo = None;
        self.auth_user = None;
        self.state = State::Command;
        self.reset_transaction();
    }

    fn track_errors(&mut self, action: Action) -> Action {
        let failed = match &action {
            Action::Reply(reply) => reply.code >= 500,
            _ => false,
        };
        if failed {
            self.errors += 1;
            if self.errors >= MAX_ERRORS {
                return Action::Close(Reply::new(
                    421,
                    format!("4.7.0 {} Error: too many errors", self.config.hostname),
                ));
            }
        }
        action
    }

    fn handle_command(&mut self, line: &[u8]) -> Action {
        let text = String::from_utf8_lossy(line);
        let (verb, args) = match text.split_once(' ') {
            Some((verb, args)) => (verb, args.trim()),
            None => (text.as_ref(), ""),
        };

        match verb.to_ascii_uppercase().as_str() {
            "EHLO" => self.helo(args, true),
            "HELO" => self.helo(args, false),
            "MAIL" => self.mail(args),
            "RCPT" => self.rcpt(args),
            "DATA" => self.data(args),
            "BDAT" => self.bdat(args),
            "RSET" => {
                self.reset_transaction();
                ok("2.0.0 Ok")
            }
            "NOOP" => ok("2.0.0 Ok"),
            "QUIT" => Action::Close(Reply::new(221, "2.0.0 Bye")),
            "VRFY" => Action::Reply(Reply::new(
                252,
                "2.5.0 Cannot VRFY user, but will accept message and attempt delivery",
            )),
            "EXPN" => reply(502, "5.5.1 Error: command not implemented"),
            "HELP" => Action::Reply(Reply::new(
                214,
                "2.0.0 Commands: EHLO HELO MAIL RCPT DATA BDAT RSET NOOP QUIT VRFY AUTH STARTTLS",
            )),
            "AUTH" => self.auth(args),
            "STARTTLS" => self.starttls(args),
            "" => reply(500, "5.5.2 Error: bad syntax"),
            _ => reply(500, "5.5.2 Error: command not recognized"),
        }
    }

    fn helo(&mut self, args: &str, extended: bool) -> Action {
        let Some(domain) = args.split_whitespace().next() else {
            let verb = if extended { "EHLO" } else { "HELO" };
            return reply(501, format!("5.5.4 Syntax: {verb} hostname"));
        };
        self.helo = Some(domain.to_string());
        self.reset_transaction();

        let greeting = format!("{} Hello {}", self.config.hostname, domain);
        if !extended {
            return Action::Reply(Reply::new(250, greeting));
        }

        let mut lines = vec![
            greeting,
            "PIPELINING".into(),
            format!("SIZE {}", self.config.max_message_size),
            "8BITMIME".into(),
            "SMTPUTF8".into(),
            "ENHANCEDSTATUSCODES".into(),
            "CHUNKING".into(),
        ];
        if self.config.tls_available && !self.tls {
            lines.push("STARTTLS".into());
        }
        if self.auth_user.is_none() && (self.tls || !self.config.require_tls) {
            lines.push("AUTH PLAIN LOGIN".into());
        }
        Action::Reply(Reply::multiline(250, lines))
    }

    fn mail(&mut self, args: &str) -> Action {
        if self.helo.is_none() {
            return reply(503, "5.5.1 Error: send HELO/EHLO first");
        }
        if self.config.require_tls && !self.tls {
            return reply(530, "5.7.0 Must issue a STARTTLS command first");
        }
        if matches!(self.config.auth, AuthPolicy::Required { .. }) && self.auth_user.is_none() {
            return reply(530, "5.7.0 Authentication required");
        }
        if self.mail_from.is_some() {
            return reply(503, "5.5.1 Error: nested MAIL command");
        }
        let Some((address, params)) = parse_path(args, "FROM:") else {
            return reply(501, "5.5.4 Syntax: MAIL FROM:<address>");
        };

        for param in params.split_whitespace() {
            let (key, value) = param.split_once('=').unwrap_or((param, ""));
            if key.eq_ignore_ascii_case("SIZE") {
                match value.parse::<usize>() {
                    Ok(size) if size > self.config.max_message_size => {
                        return Action::Reply(size_exceeded());
                    }
                    Ok(_) => {}
                    Err(_) => return reply(501, "5.5.4 Error: invalid SIZE parameter"),
                }
            }
        }

        self.mail_from = Some(address);
        ok("2.1.0 Ok")
    }

    fn rcpt(&mut self, args: &str) -> Action {
        if self.mail_from.is_none() {
            return reply(503, "5.5.1 Error: need MAIL command");
        }
        let Some((address, _params)) = parse_path(args, "TO:") else {
            return reply(501, "5.5.4 Syntax: RCPT TO:<address>");
        };
        if address.is_empty() {
            return reply(501, "5.1.3 Error: empty recipient address");
        }
        if self.rcpt_to.len() >= self.config.max_recipients {
            return reply(452, "4.5.3 Error: too many recipients");
        }
        self.rcpt_to.push(address);
        ok("2.1.5 Ok")
    }

    fn data(&mut self, args: &str) -> Action {
        if !args.is_empty() {
            return reply(501, "5.5.4 Syntax: DATA");
        }
        if self.chunks.is_some() {
            return reply(503, "5.5.1 Error: DATA not allowed during BDAT transfer");
        }
        if self.mail_from.is_none() {
            return reply(503, "5.5.1 Error: need MAIL command");
        }
        if self.rcpt_to.is_empty() {
            return reply(554, "5.5.1 Error: no valid recipients");
        }
        Action::ReadData(Reply::new(354, "End data with <CR><LF>.<CR><LF>"))
    }

    fn bdat(&mut self, args: &str) -> Action {
        let mut parts = args.split_whitespace();
        let size = parts.next().and_then(|s| s.parse::<usize>().ok());
        let last = match parts.next() {
            None => false,
            Some(word) if word.eq_ignore_ascii_case("LAST") => true,
            Some(_) => return reply(501, "5.5.4 Syntax: BDAT size [LAST]"),
        };
        let Some(size) = size else {
            return reply(501, "5.5.4 Syntax: BDAT size [LAST]");
        };

        // The chunk is always read so the connection stays in sync; a rejected
        // command is answered once its data has been consumed (RFC 3030 §2).
        if self.mail_from.is_none() {
            self.chunk_error = Some(Reply::new(503, "5.5.1 Error: need MAIL command"));
        } else if self.rcpt_to.is_empty() {
            self.chunk_error = Some(Reply::new(554, "5.5.1 Error: no valid recipients"));
        } else {
            self.chunks.get_or_insert_with(Vec::new);
        }
        Action::ReadChunk { size, last }
    }

    fn starttls(&mut self, args: &str) -> Action {
        if !args.is_empty() {
            return reply(501, "5.5.4 Syntax: STARTTLS");
        }
        if self.tls {
            return reply(503, "5.5.1 Error: TLS already active");
        }
        if !self.config.tls_available {
            return reply(454, "4.7.0 TLS not available");
        }
        Action::StartTls(Reply::new(220, "2.0.0 Ready to start TLS"))
    }

    fn auth(&mut self, args: &str) -> Action {
        if self.helo.is_none() {
            return reply(503, "5.5.1 Error: send EHLO first");
        }
        if self.auth_user.is_some() {
            return reply(503, "5.5.1 Error: already authenticated");
        }
        if self.mail_from.is_some() {
            return reply(
                503,
                "5.5.1 Error: AUTH not allowed during a mail transaction",
            );
        }
        if self.config.require_tls && !self.tls {
            return reply(
                538,
                "5.7.11 Encryption required for requested authentication mechanism",
            );
        }

        let mut parts = args.split_whitespace();
        let Some(mechanism) = parts.next() else {
            return reply(501, "5.5.4 Syntax: AUTH mechanism");
        };
        let initial = parts.next();

        match mechanism.to_ascii_uppercase().as_str() {
            "PLAIN" => match initial {
                Some(response) => self.finish_auth_plain(response.as_bytes()),
                None => {
                    self.state = State::AuthPlain;
                    Action::Reply(Reply::new(334, ""))
                }
            },
            "LOGIN" => match initial {
                Some(username) => self.auth_login_username(username.as_bytes()),
                None => {
                    self.state = State::AuthLoginUsername;
                    // "Username:"
                    Action::Reply(Reply::new(334, "VXNlcm5hbWU6"))
                }
            },
            _ => reply(504, "5.5.4 Error: unrecognized authentication type"),
        }
    }

    fn finish_auth_plain(&mut self, line: &[u8]) -> Action {
        if line == b"*" {
            return auth_cancelled();
        }
        let Some(decoded) = decode_base64(line) else {
            return invalid_base64();
        };
        // authzid NUL authcid NUL passwd
        let mut fields = decoded.split(|&b| b == 0);
        let (Some(_authzid), Some(username), Some(password), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            return reply(501, "5.5.2 Error: malformed AUTH PLAIN response");
        };
        let username = String::from_utf8_lossy(username).into_owned();
        let password = String::from_utf8_lossy(password).into_owned();
        self.verify_credentials(username, &password)
    }

    fn auth_login_username(&mut self, line: &[u8]) -> Action {
        if line == b"*" {
            return auth_cancelled();
        }
        let Some(decoded) = decode_base64(line) else {
            return invalid_base64();
        };
        self.state = State::AuthLoginPassword {
            username: String::from_utf8_lossy(&decoded).into_owned(),
        };
        // "Password:"
        Action::Reply(Reply::new(334, "UGFzc3dvcmQ6"))
    }

    fn auth_login_password(&mut self, username: String, line: &[u8]) -> Action {
        if line == b"*" {
            return auth_cancelled();
        }
        let Some(decoded) = decode_base64(line) else {
            return invalid_base64();
        };
        let password = String::from_utf8_lossy(&decoded).into_owned();
        self.verify_credentials(username, &password)
    }

    fn verify_credentials(&mut self, username: String, password: &str) -> Action {
        let accepted = match &self.config.auth {
            AuthPolicy::AcceptAny => true,
            AuthPolicy::Required {
                username: expected_user,
                password: expected_password,
            } => *expected_user == username && expected_password == password,
        };
        if accepted {
            self.auth_user = Some(username);
            Action::Reply(Reply::new(235, "2.7.0 Authentication successful"))
        } else {
            reply(535, "5.7.8 Error: authentication credentials invalid")
        }
    }

    fn take_envelope(&mut self, data: Vec<u8>) -> Envelope {
        let envelope = Envelope {
            helo: self.helo.clone(),
            mail_from: self.mail_from.take().unwrap_or_default(),
            rcpt_to: std::mem::take(&mut self.rcpt_to),
            auth_user: self.auth_user.clone(),
            tls: self.tls,
            data,
        };
        self.reset_transaction();
        envelope
    }

    fn reset_transaction(&mut self) {
        self.mail_from = None;
        self.rcpt_to.clear();
        self.chunks = None;
        self.chunk_error = None;
    }
}

fn reply(code: u16, text: impl Into<String>) -> Action {
    Action::Reply(Reply::new(code, text))
}

fn ok(text: &str) -> Action {
    reply(250, text)
}

fn size_exceeded() -> Reply {
    Reply::new(
        552,
        "5.3.4 Error: message exceeds fixed maximum message size",
    )
}

fn auth_cancelled() -> Action {
    reply(501, "5.0.0 Error: authentication cancelled")
}

fn invalid_base64() -> Action {
    reply(501, "5.5.2 Error: invalid base64 data")
}

fn decode_base64(line: &[u8]) -> Option<Vec<u8>> {
    let line = line.trim_ascii();
    if line == b"=" {
        return Some(Vec::new());
    }
    BASE64.decode(line).ok()
}

fn trim_line_ending(line: &[u8]) -> &[u8] {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    line.strip_suffix(b"\r").unwrap_or(line)
}

/// Parse `FROM:<path> params` / `TO:<path> params`, returning the address and
/// the remaining ESMTP parameters. The null path `<>` yields an empty address.
fn parse_path<'a>(args: &'a str, keyword: &str) -> Option<(String, &'a str)> {
    let prefix = args.get(..keyword.len())?;
    if !prefix.eq_ignore_ascii_case(keyword) {
        return None;
    }
    let rest = args[keyword.len()..].trim_start();

    let (path, params) = if let Some(inner) = rest.strip_prefix('<') {
        let end = inner.find('>')?;
        (&inner[..end], &inner[end + 1..])
    } else {
        // Be lenient with clients that omit the angle brackets.
        match rest.split_once(char::is_whitespace) {
            Some((path, params)) => (path, params),
            None => (rest, ""),
        }
    };

    // Strip an obsolete source route: <@a,@b:user@example.com>
    let address = match path.strip_prefix('@') {
        Some(_) => path.split_once(':').map_or(path, |(_, address)| address),
        None => path,
    };
    if address.contains(char::is_whitespace) {
        return None;
    }
    Some((address.to_string(), params.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        Session::new(Arc::new(SessionConfig::default()))
    }

    fn session_with(config: SessionConfig) -> Session {
        Session::new(Arc::new(config))
    }

    fn code(action: Action) -> u16 {
        match action {
            Action::Reply(r) | Action::ReadData(r) | Action::StartTls(r) | Action::Close(r) => {
                r.code
            }
            Action::ReadChunk { .. } => 0,
        }
    }

    fn send(session: &mut Session, line: &str) -> u16 {
        code(session.handle_line(format!("{line}\r\n").as_bytes()))
    }

    #[test]
    fn full_transaction() {
        let mut s = session();
        assert_eq!(send(&mut s, "EHLO client.example"), 250);
        assert_eq!(send(&mut s, "MAIL FROM:<alice@example.com> SIZE=100"), 250);
        assert_eq!(send(&mut s, "RCPT TO:<bob@example.com>"), 250);
        assert_eq!(send(&mut s, "RCPT TO:<carol@example.com>"), 250);
        assert_eq!(send(&mut s, "DATA"), 354);

        let envelope = s
            .handle_data(b"Subject: hi\r\n\r\nbody\r\n".to_vec(), false)
            .unwrap();
        assert_eq!(envelope.mail_from, "alice@example.com");
        assert_eq!(
            envelope.rcpt_to,
            vec!["bob@example.com", "carol@example.com"]
        );
        assert_eq!(envelope.helo.as_deref(), Some("client.example"));

        // The transaction resets, but the session stays greeted.
        assert_eq!(send(&mut s, "RCPT TO:<bob@example.com>"), 503);
        assert_eq!(send(&mut s, "MAIL FROM:<>"), 250);
        assert_eq!(send(&mut s, "QUIT"), 221);
    }

    #[test]
    fn ehlo_advertises_extensions() {
        let mut s = session();
        let Action::Reply(reply) = s.handle_line(b"EHLO x\r\n") else {
            panic!("expected reply");
        };
        let wire = reply.to_wire();
        for ext in [
            "PIPELINING",
            "SIZE 26214400",
            "8BITMIME",
            "SMTPUTF8",
            "CHUNKING",
            "AUTH PLAIN LOGIN",
        ] {
            assert!(wire.contains(ext), "missing {ext} in {wire}");
        }
        assert!(!wire.contains("STARTTLS"));
    }

    #[test]
    fn commands_are_case_insensitive_and_lenient() {
        let mut s = session();
        assert_eq!(send(&mut s, "ehlo x"), 250);
        assert_eq!(send(&mut s, "mail from: <a@b.c>"), 250);
        assert_eq!(send(&mut s, "rcpt to:d@e.f"), 250);
        assert_eq!(send(&mut s, "data"), 354);
    }

    #[test]
    fn enforces_command_order() {
        let mut s = session();
        assert_eq!(send(&mut s, "MAIL FROM:<a@b.c>"), 503);
        assert_eq!(send(&mut s, "HELO x"), 250);
        assert_eq!(send(&mut s, "RCPT TO:<a@b.c>"), 503);
        assert_eq!(send(&mut s, "DATA"), 503);
        assert_eq!(send(&mut s, "MAIL FROM:<a@b.c>"), 250);
        assert_eq!(send(&mut s, "MAIL FROM:<a@b.c>"), 503);
        assert_eq!(send(&mut s, "DATA"), 554);
        assert_eq!(send(&mut s, "RSET"), 250);
        assert_eq!(send(&mut s, "RCPT TO:<a@b.c>"), 503);
    }

    #[test]
    fn rejects_bad_syntax() {
        let mut s = session();
        assert_eq!(send(&mut s, "EHLO"), 501);
        assert_eq!(send(&mut s, "EHLO x"), 250);
        assert_eq!(send(&mut s, "MAIL TO:<a@b.c>"), 501);
        assert_eq!(send(&mut s, "MAIL FROM:<a@b.c"), 501);
        assert_eq!(send(&mut s, "BOGUS"), 500);
        assert_eq!(send(&mut s, "MAIL FROM:<a@b.c>"), 250);
        assert_eq!(send(&mut s, "RCPT TO:<>"), 501);
    }

    #[test]
    fn strips_source_routes() {
        let (address, _) = parse_path("TO:<@relay.example:bob@example.com>", "TO:").unwrap();
        assert_eq!(address, "bob@example.com");
    }

    #[test]
    fn rejects_declared_size_over_limit() {
        let mut s = session_with(SessionConfig {
            max_message_size: 10,
            ..Default::default()
        });
        send(&mut s, "EHLO x");
        assert_eq!(send(&mut s, "MAIL FROM:<a@b.c> SIZE=11"), 552);
        assert_eq!(send(&mut s, "MAIL FROM:<a@b.c> SIZE=10"), 250);
    }

    #[test]
    fn oversized_data_is_rejected() {
        let mut s = session();
        send(&mut s, "EHLO x");
        send(&mut s, "MAIL FROM:<a@b.c>");
        send(&mut s, "RCPT TO:<d@e.f>");
        send(&mut s, "DATA");
        let reply = s.handle_data(Vec::new(), true).unwrap_err();
        assert_eq!(reply.code, 552);
        assert_eq!(send(&mut s, "RCPT TO:<d@e.f>"), 503);
    }

    #[test]
    fn bdat_chunks_are_assembled() {
        let mut s = session();
        send(&mut s, "EHLO x");
        send(&mut s, "MAIL FROM:<a@b.c>");
        send(&mut s, "RCPT TO:<d@e.f>");
        assert_eq!(
            s.handle_line(b"BDAT 6\r\n"),
            Action::ReadChunk {
                size: 6,
                last: false
            }
        );
        assert!(matches!(
            s.handle_chunk(b"Hello ".to_vec(), false, false),
            ChunkOutcome::Reply(Reply { code: 250, .. })
        ));
        assert_eq!(send(&mut s, "DATA"), 503);
        assert_eq!(
            s.handle_line(b"BDAT 5 LAST\r\n"),
            Action::ReadChunk {
                size: 5,
                last: true
            }
        );
        let ChunkOutcome::Deliver(envelope) = s.handle_chunk(b"World".to_vec(), true, false) else {
            panic!("expected delivery");
        };
        assert_eq!(envelope.data, b"Hello World");
    }

    #[test]
    fn rejected_bdat_still_consumes_chunk() {
        let mut s = session();
        send(&mut s, "EHLO x");
        assert_eq!(
            s.handle_line(b"BDAT 3 LAST\r\n"),
            Action::ReadChunk {
                size: 3,
                last: true
            }
        );
        assert!(matches!(
            s.handle_chunk(b"abc".to_vec(), true, false),
            ChunkOutcome::Reply(Reply { code: 503, .. })
        ));
    }

    #[test]
    fn auth_plain_initial_response() {
        let mut s = session();
        send(&mut s, "EHLO x");
        // \0user\0secret
        assert_eq!(send(&mut s, "AUTH PLAIN AHVzZXIAc2VjcmV0"), 235);
        assert_eq!(send(&mut s, "AUTH PLAIN AHVzZXIAc2VjcmV0"), 503);
        send(&mut s, "MAIL FROM:<a@b.c>");
        send(&mut s, "RCPT TO:<d@e.f>");
        send(&mut s, "DATA");
        let envelope = s.handle_data(Vec::new(), false).unwrap();
        assert_eq!(envelope.auth_user.as_deref(), Some("user"));
    }

    #[test]
    fn auth_login_challenge_flow() {
        let mut s = session_with(SessionConfig {
            auth: AuthPolicy::Required {
                username: "user".into(),
                password: "secret".into(),
            },
            ..Default::default()
        });
        send(&mut s, "EHLO x");
        assert_eq!(send(&mut s, "MAIL FROM:<a@b.c>"), 530);
        assert_eq!(send(&mut s, "AUTH LOGIN"), 334);
        assert_eq!(send(&mut s, "dXNlcg=="), 334); // user
        assert_eq!(send(&mut s, "d3Jvbmc="), 535); // wrong
        assert_eq!(send(&mut s, "AUTH LOGIN dXNlcg=="), 334);
        assert_eq!(send(&mut s, "c2VjcmV0"), 235); // secret
        assert_eq!(send(&mut s, "MAIL FROM:<a@b.c>"), 250);
    }

    #[test]
    fn auth_can_be_cancelled() {
        let mut s = session();
        send(&mut s, "EHLO x");
        assert_eq!(send(&mut s, "AUTH PLAIN"), 334);
        assert_eq!(send(&mut s, "*"), 501);
        assert_eq!(send(&mut s, "NOOP"), 250);
    }

    #[test]
    fn starttls_flow() {
        let mut s = session_with(SessionConfig {
            tls_available: true,
            require_tls: true,
            ..Default::default()
        });
        assert_eq!(send(&mut s, "EHLO x"), 250);
        assert_eq!(send(&mut s, "MAIL FROM:<a@b.c>"), 530);
        assert!(matches!(
            s.handle_line(b"STARTTLS\r\n"),
            Action::StartTls(_)
        ));
        s.tls_established();
        // The client must greet again after the handshake.
        assert_eq!(send(&mut s, "MAIL FROM:<a@b.c>"), 503);
        assert_eq!(send(&mut s, "EHLO x"), 250);
        assert_eq!(send(&mut s, "STARTTLS"), 503);
        assert_eq!(send(&mut s, "MAIL FROM:<a@b.c>"), 250);
    }

    #[test]
    fn starttls_unavailable() {
        let mut s = session();
        send(&mut s, "EHLO x");
        assert_eq!(send(&mut s, "STARTTLS"), 454);
    }

    #[test]
    fn too_many_errors_closes_connection() {
        let mut s = session();
        for _ in 0..MAX_ERRORS - 1 {
            assert_eq!(send(&mut s, "BOGUS"), 500);
        }
        assert!(matches!(
            s.handle_line(b"BOGUS\r\n"),
            Action::Close(Reply { code: 421, .. })
        ));
    }
}
