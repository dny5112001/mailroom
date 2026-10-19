# Contributing to Mailroom

Thanks for helping! Issues and pull requests are welcome.

## Getting started

```bash
cargo test                       # unit tests + end-to-end tests against a real server
MAILROOM_UI_DIR=ui cargo run     # UI files are read from disk on every request
python3 scripts/send-samples.py  # fill the inbox with realistic emails
```

## Where things live

- **SMTP protocol rules** go in `src/smtp/session.rs`. It is sans-IO: feed it lines, assert on the replies. Every behaviour change there should come with a unit test in the same file.
- **Network behaviour** (timeouts, TLS, framing) is in `src/smtp/server.rs` and is covered by `tests/e2e.rs`.
- **What a message means** (bodies, attachments, Bcc detection, search text) is decided in `src/email/parser.rs`; link and code extraction is in `src/email/extract.rs`.
- **SQL** stays in `src/storage/repo.rs`; the search language is in `src/storage/query.rs`.
- **The web UI** is plain HTML, CSS and ES modules in `ui/` — no build step. Interpolate untrusted values only through the `html` tagged template, which escapes them.

## Before opening a pull request

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

Keep the binary small and dependency-free at runtime: prefer the standard library or an existing dependency over adding a new crate.
