use std::{io::IsTerminal, path::Path, process::ExitCode};

use clap::Parser;
use mailroom::{Config, Running};
use tracing_subscriber::EnvFilter;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();

    // Installed as (or symlinked to) `sendmail`, or run as `mailroom sendmail`.
    let invoked_as_sendmail = args
        .first()
        .and_then(|arg0| Path::new(arg0).file_name())
        .is_some_and(|name| name == "sendmail");
    if invoked_as_sendmail {
        return ExitCode::from(mailroom::sendmail::main(&args[1..]));
    }
    if args.get(1).map(String::as_str) == Some("sendmail") {
        return ExitCode::from(mailroom::sendmail::main(&args[2..]));
    }

    let config = Config::parse();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_new(&config.log).unwrap_or_else(|_| EnvFilter::new("info")))
        .with_target(false)
        .init();

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("error: could not start runtime: {err}");
            return ExitCode::FAILURE;
        }
    };

    let result = runtime.block_on(async {
        let running = mailroom::start(&config).await?;
        print_banner(&config, &running);
        running.wait().await
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn print_banner(config: &Config, running: &Running) {
    let storage = match &config.db {
        Some(path) => path.display().to_string(),
        None => "in memory".into(),
    };
    let limit = match config.max_messages {
        0 => "unlimited".to_string(),
        n => format!("newest {n} kept"),
    };
    let mut smtp_notes = Vec::new();
    if config.smtp_auth.is_some() {
        smtp_notes.push("auth required");
    }
    if config.smtp_tls_cert.is_some() || config.smtp_tls_self_signed {
        smtp_notes.push(if config.smtp_require_tls {
            "STARTTLS required"
        } else {
            "STARTTLS"
        });
    }
    let smtp_notes = if smtp_notes.is_empty() {
        String::new()
    } else {
        format!("  ({})", smtp_notes.join(", "))
    };

    let color = std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    let paint = |code: &str, text: &str| {
        if color {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    };

    eprintln!();
    eprintln!(
        "  {} v{}",
        paint("1", "Mailroom"),
        env!("CARGO_PKG_VERSION")
    );
    eprintln!();
    eprintln!(
        "  {}     {}{smtp_notes}",
        paint("2", "SMTP"),
        running.smtp_addr
    );
    eprintln!(
        "  {}   {}",
        paint("2", "Web UI"),
        paint("36", &running.url())
    );
    eprintln!("  {}  {storage} ({limit})", paint("2", "Storage"));
    eprintln!();
}
