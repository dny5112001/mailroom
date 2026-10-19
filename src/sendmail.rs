//! `mailroom sendmail` — a drop-in replacement for `/usr/sbin/sendmail`.
//!
//! Reads a message on stdin and submits it to a Mailroom SMTP server, so tools
//! that only know how to pipe into sendmail (PHP's `mail()`, cron, git
//! send-email) can be captured too:
//!
//! ```ini
//! ; php.ini
//! sendmail_path = "/usr/local/bin/mailroom sendmail -t"
//! ```

use std::{
    io::{self, BufRead, BufReader, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use mail_parser::MessageParser;

/// sendmail's "temporary failure" exit status (EX_TEMPFAIL).
const EX_TEMPFAIL: u8 = 75;
const EX_USAGE: u8 = 64;

#[derive(Debug, Default, PartialEq, Eq)]
struct Options {
    from: Option<String>,
    recipients: Vec<String>,
    /// `-t`: read recipients from the To, Cc and Bcc headers.
    read_headers: bool,
    /// `-i` / `-oi`: a line with a single dot does not end the message.
    ignore_dots: bool,
    smtp: Option<String>,
}

pub fn main(args: &[String]) -> u8 {
    let options = match parse_args(args) {
        Ok(options) => options,
        Err(err) => {
            eprintln!("mailroom sendmail: {err}");
            return EX_USAGE;
        }
    };
    match run(options) {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("mailroom sendmail: {err:#}");
            EX_TEMPFAIL
        }
    }
}

fn parse_args(args: &[String]) -> Result<Options> {
    let mut options = Options::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-t" => options.read_headers = true,
            "-i" | "-oi" => options.ignore_dots = true,
            "-f" | "-r" => options.from = Some(iter.next().context("-f needs an address")?.clone()),
            "-S" | "--smtp" => {
                options.smtp = Some(iter.next().context("-S needs host:port")?.clone())
            }
            "-F" | "-O" | "-o" | "-B" | "-N" | "-R" | "-V" | "-X" | "-L" => {
                iter.next(); // option with a value we do not need
            }
            "-bs" => bail!("-bs (SMTP on stdin) is not supported"),
            "--" => options.recipients.extend(iter.by_ref().cloned()),
            flag if flag.starts_with("--smtp=") => {
                options.smtp = Some(flag["--smtp=".len()..].to_string())
            }
            flag if flag.starts_with("-f") || flag.starts_with("-r") => {
                options.from = Some(flag[2..].to_string())
            }
            flag if flag.starts_with('-') => {} // -bm, -v, -odi, ...: accepted and ignored
            recipient => options.recipients.extend(
                recipient
                    .split(',')
                    .map(str::trim)
                    .filter(|r| !r.is_empty())
                    .map(str::to_string),
            ),
        }
    }
    Ok(options)
}

fn run(options: Options) -> Result<()> {
    let original = read_message(io::stdin().lock(), options.ignore_dots)?;
    let parsed = MessageParser::default().parse(&original);

    let mut recipients = options.recipients.clone();
    if options.read_headers
        && let Some(parsed) = &parsed
    {
        for address in parsed
            .all_to()
            .chain(parsed.all_cc())
            .chain(parsed.all_bcc())
        {
            recipients.extend(
                address
                    .iter()
                    .filter_map(|a| a.address())
                    .map(str::to_string),
            );
        }
    }
    if recipients.is_empty() {
        bail!("no recipients given (pass addresses or use -t)");
    }

    let from = options
        .from
        .clone()
        .or_else(|| {
            parsed
                .as_ref()
                .and_then(|p| p.from())
                .and_then(|a| a.first())
                .and_then(|a| a.address())
                .map(str::to_string)
        })
        .unwrap_or_else(default_sender);
    drop(parsed);

    let message = if options.read_headers {
        strip_bcc(&original)
    } else {
        original
    };
    let target = options
        .smtp
        .clone()
        .or_else(|| std::env::var("MAILROOM_SENDMAIL_SMTP").ok())
        .unwrap_or_else(|| "127.0.0.1:1025".into());
    submit(&target, &from, &recipients, &message)
}

fn default_sender() -> String {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "sendmail".into());
    format!("{user}@localhost")
}

/// Read the message from stdin. Unless `ignore_dots`, a line holding a single
/// `.` ends the message, as with traditional sendmail.
fn read_message(input: impl Read, ignore_dots: bool) -> Result<Vec<u8>> {
    let mut reader = BufReader::new(input);
    let mut message = Vec::new();
    let mut line = Vec::new();
    loop {
        line.clear();
        if reader.read_until(b'\n', &mut line)? == 0 {
            break;
        }
        if !ignore_dots && matches!(line.as_slice(), b".\n" | b".\r\n" | b".") {
            break;
        }
        message.extend_from_slice(&line);
    }
    Ok(message)
}

/// Remove Bcc header fields (including folded continuation lines).
fn strip_bcc(message: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(message.len());
    let mut in_headers = true;
    let mut skipping = false;
    for line in message.split_inclusive(|&b| b == b'\n') {
        if in_headers {
            if line == b"\r\n" || line == b"\n" {
                in_headers = false;
            } else if skipping && line.first().is_some_and(|b| *b == b' ' || *b == b'\t') {
                continue;
            } else {
                skipping = line.len() >= 4 && line[..4].eq_ignore_ascii_case(b"bcc:");
                if skipping {
                    continue;
                }
            }
        }
        out.extend_from_slice(line);
    }
    out
}

struct Client {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
}

impl Client {
    fn connect(target: &str) -> Result<Self> {
        let addr = target
            .to_socket_addrs()
            .with_context(|| format!("resolving {target}"))?
            .next()
            .ok_or_else(|| anyhow!("could not resolve {target}"))?;
        let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(10))
            .with_context(|| format!("connecting to Mailroom at {target}"))?;
        stream.set_read_timeout(Some(Duration::from_secs(60)))?;
        Ok(Self {
            reader: BufReader::new(stream.try_clone()?),
            writer: stream,
        })
    }

    /// Read a (possibly multi-line) reply and check its code class.
    fn expect(&mut self, class: u8, context: &str) -> Result<()> {
        let mut text = String::new();
        loop {
            let mut line = String::new();
            if self.reader.read_line(&mut line)? == 0 {
                bail!("server closed the connection during {context}");
            }
            text.push_str(&line);
            if line.len() < 4 || line.as_bytes()[3] != b'-' {
                break;
            }
        }
        if text.as_bytes().first() != Some(&(b'0' + class)) {
            bail!("{context} rejected: {}", text.trim());
        }
        Ok(())
    }

    fn command(&mut self, line: &str, class: u8) -> Result<()> {
        write!(self.writer, "{line}\r\n")?;
        self.expect(class, line.split(':').next().unwrap_or(line))
    }
}

fn submit(target: &str, from: &str, recipients: &[String], message: &[u8]) -> Result<()> {
    let mut client = Client::connect(target)?;
    client.expect(2, "greeting")?;
    client.command("EHLO localhost", 2)?;
    client.command(&format!("MAIL FROM:<{from}>"), 2)?;
    for rcpt in recipients {
        client.command(&format!("RCPT TO:<{rcpt}>"), 2)?;
    }
    client.command("DATA", 3)?;
    client.writer.write_all(&dot_stuff(message))?;
    client.writer.write_all(b".\r\n")?;
    client.expect(2, "message")?;
    let _ = client.command("QUIT", 2);
    Ok(())
}

/// Normalize line endings to CRLF and escape lines starting with a dot.
fn dot_stuff(message: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(message.len() + 64);
    for line in message.split_inclusive(|&b| b == b'\n') {
        let content = line
            .strip_suffix(b"\n")
            .map(|l| l.strip_suffix(b"\r").unwrap_or(l))
            .unwrap_or(line);
        if content.first() == Some(&b'.') {
            out.push(b'.');
        }
        out.extend_from_slice(content);
        out.extend_from_slice(b"\r\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_common_invocations() {
        let options = parse_args(&args(&["-t", "-i", "-fsender@example.com"])).unwrap();
        assert!(options.read_headers && options.ignore_dots);
        assert_eq!(options.from.as_deref(), Some("sender@example.com"));

        let options = parse_args(&args(&[
            "-oi",
            "-f",
            "a@b.c",
            "x@y.z,w@v.u",
            "-S",
            "mail:1025",
        ]))
        .unwrap();
        assert_eq!(options.recipients, vec!["x@y.z", "w@v.u"]);
        assert_eq!(options.smtp.as_deref(), Some("mail:1025"));
    }

    #[test]
    fn single_dot_ends_input_unless_ignored() {
        let input = b"Subject: x\n\nline\n.\nafter\n";
        assert_eq!(
            read_message(&input[..], false).unwrap(),
            b"Subject: x\n\nline\n"
        );
        assert_eq!(read_message(&input[..], true).unwrap(), input.to_vec());
    }

    #[test]
    fn strips_bcc_headers() {
        let message =
            b"To: a@b.c\nBcc: secret@b.c,\n  other@b.c\nSubject: hi\n\nBcc: in body stays\n";
        assert_eq!(
            strip_bcc(message),
            b"To: a@b.c\nSubject: hi\n\nBcc: in body stays\n".to_vec()
        );
    }

    #[test]
    fn dot_stuffs_and_normalizes() {
        assert_eq!(dot_stuff(b"a\n.b\r\nc"), b"a\r\n..b\r\nc\r\n".to_vec());
    }
}
