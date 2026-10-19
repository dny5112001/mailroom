use std::path::PathBuf;

use clap::Parser;

/// A local SMTP server and web inbox for testing email.
///
/// Point your application's SMTP settings at Mailroom and every message it
/// sends is captured — never delivered — and shown in the web UI.
#[derive(Debug, Clone, Parser)]
#[command(
    name = "mailroom",
    version,
    about,
    long_about,
    after_help = "Use `mailroom sendmail [-t] [-f from] [recipients...]` as a drop-in sendmail replacement."
)]
pub struct Config {
    /// SMTP listen address.
    #[arg(
        long = "smtp",
        env = "MAILROOM_SMTP",
        default_value = "127.0.0.1:1025",
        value_name = "ADDR"
    )]
    pub smtp_addr: String,

    /// Web UI and API listen address.
    #[arg(
        long = "http",
        env = "MAILROOM_HTTP",
        default_value = "127.0.0.1:8025",
        value_name = "ADDR"
    )]
    pub http_addr: String,

    /// SQLite file to persist messages in. Without it, messages live in memory
    /// and disappear on exit.
    #[arg(long, env = "MAILROOM_DB", value_name = "PATH")]
    pub db: Option<PathBuf>,

    /// Keep at most this many messages, deleting the oldest. 0 keeps everything.
    #[arg(
        long,
        env = "MAILROOM_MAX_MESSAGES",
        default_value_t = 1000,
        value_name = "N"
    )]
    pub max_messages: usize,

    /// Largest accepted message, e.g. `25MB`, `512KB` or a byte count.
    #[arg(
        long,
        env = "MAILROOM_MAX_MESSAGE_SIZE",
        default_value = "25MB",
        value_parser = parse_size,
        value_name = "SIZE"
    )]
    pub max_message_size: usize,

    /// Hostname announced in SMTP greetings.
    #[arg(
        long,
        env = "MAILROOM_HOSTNAME",
        default_value = "mailroom",
        value_name = "NAME"
    )]
    pub hostname: String,

    /// Require SMTP clients to authenticate with these credentials. Without it,
    /// AUTH is optional and any credentials are accepted.
    #[arg(long, env = "MAILROOM_SMTP_AUTH", value_parser = parse_credentials, value_name = "USER:PASSWORD")]
    pub smtp_auth: Option<Credentials>,

    /// PEM certificate (chain) used for STARTTLS.
    #[arg(
        long,
        env = "MAILROOM_SMTP_TLS_CERT",
        requires = "smtp_tls_key",
        value_name = "PATH"
    )]
    pub smtp_tls_cert: Option<PathBuf>,

    /// PEM private key used for STARTTLS.
    #[arg(
        long,
        env = "MAILROOM_SMTP_TLS_KEY",
        requires = "smtp_tls_cert",
        value_name = "PATH"
    )]
    pub smtp_tls_key: Option<PathBuf>,

    /// Offer STARTTLS using a self-signed certificate generated at startup.
    #[arg(
        long,
        env = "MAILROOM_SMTP_TLS_SELF_SIGNED",
        conflicts_with = "smtp_tls_cert"
    )]
    pub smtp_tls_self_signed: bool,

    /// Refuse MAIL and AUTH until the client has issued STARTTLS.
    #[arg(long, env = "MAILROOM_SMTP_REQUIRE_TLS")]
    pub smtp_require_tls: bool,

    /// Serve the UI and API under a path prefix, e.g. `/mailroom/`, when behind
    /// a reverse proxy.
    #[arg(long, env = "MAILROOM_WEBROOT", default_value = "/", value_parser = parse_webroot, value_name = "PATH")]
    pub webroot: String,

    /// Allow cross-origin API requests from these origins (comma separated, or
    /// `*`). Useful when browser-based tests call the API directly.
    #[arg(
        long,
        env = "MAILROOM_CORS",
        value_delimiter = ',',
        value_name = "ORIGINS"
    )]
    pub cors: Vec<String>,

    /// Log filter, e.g. `info`, `debug` or `mailroom=trace`.
    #[arg(
        long,
        env = "MAILROOM_LOG",
        default_value = "info",
        value_name = "FILTER"
    )]
    pub log: String,
}

impl Config {
    /// Defaults suitable for tests: ephemeral ports, in-memory storage.
    pub fn ephemeral() -> Self {
        Self::parse_from(["mailroom", "--smtp", "127.0.0.1:0", "--http", "127.0.0.1:0"])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

fn parse_credentials(value: &str) -> Result<Credentials, String> {
    match value.split_once(':') {
        Some((username, password)) if !username.is_empty() => Ok(Credentials {
            username: username.to_string(),
            password: password.to_string(),
        }),
        _ => Err("expected USER:PASSWORD".into()),
    }
}

/// Parse sizes like `25MB`, `25M`, `512kb`, `1GiB` or `1048576`.
pub fn parse_size(value: &str) -> Result<usize, String> {
    let value = value.trim();
    let split = value
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(value.len());
    let (number, unit) = value.split_at(split);
    let number: f64 = number
        .parse()
        .map_err(|_| format!("invalid size {value:?}"))?;
    let multiplier: f64 = match unit.trim().to_ascii_lowercase().as_str() {
        "" | "b" => 1.0,
        "k" | "kb" | "kib" => 1024.0,
        "m" | "mb" | "mib" => 1024.0 * 1024.0,
        "g" | "gb" | "gib" => 1024.0 * 1024.0 * 1024.0,
        _ => return Err(format!("unknown size unit in {value:?}")),
    };
    let bytes = number * multiplier;
    if bytes < 1.0 {
        return Err("size must be at least 1 byte".into());
    }
    Ok(bytes as usize)
}

fn parse_webroot(value: &str) -> Result<String, String> {
    let trimmed = value.trim().trim_matches('/');
    if trimmed.contains(|c: char| c.is_whitespace() || c == '?' || c == '#') {
        return Err(format!("invalid webroot {value:?}"));
    }
    Ok(if trimmed.is_empty() {
        "/".into()
    } else {
        format!("/{trimmed}/")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sizes() {
        assert_eq!(parse_size("25MB"), Ok(25 * 1024 * 1024));
        assert_eq!(parse_size("512kb"), Ok(512 * 1024));
        assert_eq!(parse_size("1.5M"), Ok(1024 * 1024 * 3 / 2));
        assert_eq!(parse_size("1000"), Ok(1000));
        assert!(parse_size("ten").is_err());
        assert!(parse_size("10XB").is_err());
    }

    #[test]
    fn normalizes_webroot() {
        assert_eq!(parse_webroot("/").unwrap(), "/");
        assert_eq!(parse_webroot("mail").unwrap(), "/mail/");
        assert_eq!(parse_webroot("/a/b/").unwrap(), "/a/b/");
    }

    #[test]
    fn parses_credentials() {
        let creds = parse_credentials("user:pa:ss").unwrap();
        assert_eq!(creds.username, "user");
        assert_eq!(creds.password, "pa:ss");
        assert!(parse_credentials(":x").is_err());
    }

    #[test]
    fn cli_is_valid() {
        use clap::CommandFactory;
        Config::command().debug_assert();
    }
}
