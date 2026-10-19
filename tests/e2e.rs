//! End-to-end tests: real SMTP clients talk to a running server and the
//! results are checked through the HTTP API.

use std::{
    io::Write,
    process::{Command, Stdio},
    time::Duration,
};

use futures_util::StreamExt;
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{Attachment, MultiPart, SinglePart, header::ContentType},
    transport::smtp::{
        authentication::Credentials,
        client::{Tls, TlsParameters},
    },
};
use mailroom::{Config, Running};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
};

async fn start_with(args: &[&str]) -> Running {
    let mut argv = vec!["mailroom", "--smtp", "127.0.0.1:0", "--http", "127.0.0.1:0"];
    argv.extend_from_slice(args);
    let config = <Config as clap::Parser>::parse_from(argv);
    mailroom::start(&config).await.expect("server starts")
}

async fn start() -> Running {
    start_with(&[]).await
}

fn transport(server: &Running) -> AsyncSmtpTransport<Tokio1Executor> {
    AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous("127.0.0.1")
        .port(server.smtp_addr.port())
        .build()
}

fn api(server: &Running, path: &str) -> String {
    format!("{}api/v1/{path}", server.url())
}

async fn get_json(url: &str) -> Value {
    let response = reqwest::get(url).await.expect("request succeeds");
    assert!(
        response.status().is_success(),
        "GET {url} returned {}",
        response.status()
    );
    response.json().await.expect("json body")
}

fn welcome_message() -> Message {
    Message::builder()
        .from("Acme <noreply@acme.test>".parse().unwrap())
        .to("Alice <alice@example.com>".parse().unwrap())
        .bcc("audit@acme.test".parse().unwrap())
        .subject("Confirm your account")
        .multipart(
            MultiPart::mixed()
                .multipart(MultiPart::alternative_plain_html(
                    "Your verification code is 318264. Confirm: https://acme.test/confirm?token=abc".to_string(),
                    r#"<p>Your verification code is <b>318264</b>.</p><a href="https://acme.test/confirm?token=abc">Confirm account</a>"#.to_string(),
                ))
                .singlepart(
                    Attachment::new("terms.txt".into())
                        .body(b"terms and conditions".to_vec(), ContentType::TEXT_PLAIN),
                ),
        )
        .unwrap()
}

#[tokio::test]
async fn smtp_message_is_available_through_the_api() {
    let server = start().await;
    transport(&server)
        .send(welcome_message())
        .await
        .expect("send succeeds");

    let list = get_json(&api(&server, "messages")).await;
    assert_eq!(list["total"], 1);
    assert_eq!(list["unread"], 1);
    let summary = &list["messages"][0];
    assert_eq!(summary["subject"], "Confirm your account");
    assert_eq!(summary["from"]["address"], "noreply@acme.test");
    assert_eq!(summary["to"][0]["name"], "Alice");
    assert_eq!(summary["bcc"][0]["address"], "audit@acme.test");
    assert_eq!(summary["attachment_count"], 1);
    let id = summary["id"].as_str().unwrap().to_string();

    let detail = get_json(&api(&server, &format!("messages/{id}"))).await;
    assert!(detail["text"].as_str().unwrap().contains("318264"));
    assert!(detail["html"].as_str().unwrap().contains("<b>318264</b>"));
    assert_eq!(detail["codes"], json!(["318264"]));
    assert_eq!(
        detail["links"][0]["url"],
        "https://acme.test/confirm?token=abc"
    );
    assert_eq!(detail["links"][0]["text"], "Confirm account");
    assert_eq!(detail["envelope"]["source"], "smtp");
    assert_eq!(detail["envelope"]["mail_from"], "noreply@acme.test");
    assert_eq!(
        detail["envelope"]["rcpt_to"],
        json!(["alice@example.com", "audit@acme.test"])
    );
    assert_eq!(detail["attachments"][0]["filename"], "terms.txt");

    // Attachment download.
    let url = format!(
        "http://{}{}",
        server.http_addr,
        detail["attachments"][0]["url"].as_str().unwrap()
    );
    let body = reqwest::get(&url).await.unwrap().text().await.unwrap();
    assert_eq!(body, "terms and conditions");

    // Raw source and sandboxed HTML preview.
    let raw = reqwest::get(api(&server, &format!("messages/{id}/raw")))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(raw.contains("Subject: Confirm your account"));
    let preview = reqwest::get(api(&server, &format!("messages/{id}/html")))
        .await
        .unwrap();
    let csp = preview.headers()["content-security-policy"]
        .to_str()
        .unwrap()
        .to_string();
    assert!(csp.contains("default-src 'none'") && csp.contains("sandbox"));
    assert!(
        preview
            .text()
            .await
            .unwrap()
            .starts_with(r#"<base target="_blank">"#)
    );

    // `latest` with a query, used by tests to grab "the email I just triggered".
    let links = get_json(&api(
        &server,
        "messages/latest/links?query=to:alice@example.com",
    ))
    .await;
    assert_eq!(links["id"], id.as_str());
    let missing = reqwest::get(api(&server, "messages/latest?query=to:nobody"))
        .await
        .unwrap();
    assert_eq!(missing.status(), 404);

    server.shutdown();
}

#[tokio::test]
async fn search_read_state_and_delete() {
    let server = start().await;
    let client = reqwest::Client::new();
    for (to, subject) in [
        ("bob@example.com", "Invoice #1"),
        ("carol@example.com", "Welcome"),
        ("bob@example.com", "Reset password"),
    ] {
        let response = client
            .post(api(&server, "send"))
            .json(&json!({ "from": "app@acme.test", "to": [to], "subject": subject, "text": "hello" }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 201);
    }

    let bob = get_json(&api(&server, "messages?query=to:bob")).await;
    assert_eq!(bob["matched"], 2);
    assert_eq!(bob["total"], 3);
    let reset = get_json(&api(&server, "messages?query=subject:%22reset password%22")).await;
    assert_eq!(reset["matched"], 1);
    let not_bob = get_json(&api(&server, "messages?query=-to:bob")).await;
    assert_eq!(not_bob["messages"][0]["subject"], "Welcome");

    // Mark everything to bob as read.
    let marked: Value = client
        .post(api(&server, "messages/read"))
        .json(&json!({ "query": "to:bob" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(marked["updated"], 2);
    let unread = get_json(&api(&server, "messages?query=is:unread")).await;
    assert_eq!(unread["matched"], 1);
    assert_eq!(unread["unread"], 1);

    // Delete by query, then one by id, then clear.
    let deleted: Value = client
        .delete(api(&server, "messages?query=subject:invoice"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(deleted["deleted"], 1);
    let remaining = get_json(&api(&server, "messages")).await;
    let id = remaining["messages"][0]["id"].as_str().unwrap();
    let response = client
        .delete(api(&server, &format!("messages/{id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 204);
    client
        .delete(api(&server, "messages"))
        .send()
        .await
        .unwrap();
    assert_eq!(get_json(&api(&server, "messages")).await["total"], 0);

    server.shutdown();
}

#[tokio::test]
async fn wait_returns_message_that_arrives_later() {
    let server = start().await;
    let url = api(
        &server,
        "messages/wait?query=to:dave@example.com&timeout=10",
    );
    let waiter = tokio::spawn(async move {
        reqwest::get(url)
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()
    });

    tokio::time::sleep(Duration::from_millis(200)).await;
    let other = Message::builder()
        .from("a@acme.test".parse().unwrap())
        .to("someone@example.com".parse().unwrap())
        .subject("Not this one")
        .body("x".to_string())
        .unwrap();
    transport(&server).send(other).await.unwrap();
    let wanted = Message::builder()
        .from("a@acme.test".parse().unwrap())
        .to("dave@example.com".parse().unwrap())
        .subject("Your magic link")
        .body("Sign in: https://acme.test/magic/xyz".to_string())
        .unwrap();
    transport(&server).send(wanted).await.unwrap();

    let message = tokio::time::timeout(Duration::from_secs(5), waiter)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(message["subject"], "Your magic link");
    assert_eq!(message["links"][0]["url"], "https://acme.test/magic/xyz");

    let timed_out = reqwest::get(api(&server, "messages/wait?query=to:nobody&timeout=0.2"))
        .await
        .unwrap();
    assert_eq!(timed_out.status(), 408);
    server.shutdown();
}

#[tokio::test]
async fn sse_stream_announces_new_messages() {
    let server = start().await;
    let response = reqwest::get(api(&server, "events")).await.unwrap();
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    let mut stream = response.bytes_stream();

    let client = reqwest::Client::new();
    client
        .post(api(&server, "send"))
        .json(&json!({ "from": "a@acme.test", "to": ["b@example.com"], "subject": "Live!", "html": "<p>hi</p>" }))
        .send()
        .await
        .unwrap();

    let mut received = String::new();
    while !received.contains("event: stats") {
        let chunk = tokio::time::timeout(Duration::from_secs(5), stream.next())
            .await
            .expect("event arrives")
            .unwrap()
            .unwrap();
        received.push_str(&String::from_utf8_lossy(&chunk));
    }
    assert!(received.contains("event: message.created"));
    assert!(received.contains("\"subject\":\"Live!\""));
    server.shutdown();
}

#[tokio::test]
async fn auth_can_be_required() {
    let server = start_with(&["--smtp-auth", "app:s3cret"]).await;
    let message = || {
        Message::builder()
            .from("a@acme.test".parse().unwrap())
            .to("b@example.com".parse().unwrap())
            .subject("Authenticated")
            .body("x".to_string())
            .unwrap()
    };

    let anonymous = transport(&server).send(message()).await;
    assert!(anonymous.is_err(), "unauthenticated send should fail");

    let wrong = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous("127.0.0.1")
        .port(server.smtp_addr.port())
        .credentials(Credentials::new("app".into(), "nope".into()))
        .build();
    assert!(wrong.send(message()).await.is_err());

    let authed = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous("127.0.0.1")
        .port(server.smtp_addr.port())
        .credentials(Credentials::new("app".into(), "s3cret".into()))
        .build();
    authed
        .send(message())
        .await
        .expect("authenticated send succeeds");

    let detail = get_json(&api(&server, "messages/latest")).await;
    assert_eq!(detail["envelope"]["auth_user"], "app");
    server.shutdown();
}

#[tokio::test]
async fn starttls_with_self_signed_certificate() {
    let server = start_with(&["--smtp-tls-self-signed", "--smtp-require-tls"]).await;
    let message = Message::builder()
        .from("a@acme.test".parse().unwrap())
        .to("b@example.com".parse().unwrap())
        .subject("Encrypted")
        .singlepart(SinglePart::plain("secret".to_string()))
        .unwrap();

    let plain = transport(&server).send(message.clone()).await;
    assert!(plain.is_err(), "plaintext MAIL should be refused");

    let tls = TlsParameters::builder("localhost".into())
        .dangerous_accept_invalid_certs(true)
        .build_rustls()
        .unwrap();
    let secure = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous("127.0.0.1")
        .port(server.smtp_addr.port())
        .tls(Tls::Required(tls))
        .credentials(Credentials::new("any".into(), "thing".into()))
        .build();
    secure.send(message).await.expect("STARTTLS send succeeds");

    let detail = get_json(&api(&server, "messages/latest")).await;
    assert_eq!(detail["envelope"]["tls"], true);
    assert_eq!(detail["subject"], "Encrypted");
    server.shutdown();
}

/// Send a whole conversation in one write, as a pipelining client may.
#[tokio::test]
async fn pipelined_conversation_with_dot_stuffing_and_bdat() {
    let server = start().await;
    let stream = TcpStream::connect(server.smtp_addr).await.unwrap();
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();

    write
        .write_all(
            b"EHLO pipeline.test\r\n\
              MAIL FROM:<pipe@acme.test>\r\n\
              RCPT TO:<x@example.com>\r\n\
              DATA\r\n\
              Subject: Pipelined\r\n\r\n\
              ..leading dot\r\n\
              .\r\n\
              MAIL FROM:<pipe@acme.test>\r\n\
              RCPT TO:<y@example.com>\r\n\
              BDAT 27 LAST\r\n\
              Subject: Chunked\r\n\r\nchunks!\
              QUIT\r\n",
        )
        .await
        .unwrap();

    let mut transcript = Vec::new();
    while let Some(line) = lines.next_line().await.unwrap() {
        transcript.push(line);
    }
    let codes: Vec<&str> = transcript
        .iter()
        .filter(|l| l.as_bytes().get(3) == Some(&b' '))
        .map(|l| &l[..3])
        .collect();
    assert_eq!(
        codes,
        [
            "220", "250", "250", "250", "354", "250", "250", "250", "250", "221"
        ],
        "{transcript:#?}"
    );

    let list = get_json(&api(&server, "messages")).await;
    assert_eq!(list["total"], 2);
    assert_eq!(list["messages"][0]["subject"], "Chunked");
    let pipelined = list["messages"][1]["id"].as_str().unwrap();
    let detail = get_json(&api(&server, &format!("messages/{pipelined}"))).await;
    assert_eq!(detail["text"], ".leading dot\r\n");
    server.shutdown();
}

#[tokio::test]
async fn oversized_messages_are_rejected() {
    let server = start_with(&["--max-message-size", "1KB"]).await;
    let message = Message::builder()
        .from("a@acme.test".parse().unwrap())
        .to("b@example.com".parse().unwrap())
        .subject("Too big")
        .body("x".repeat(4096))
        .unwrap();
    let error = transport(&server).send(message).await.unwrap_err();
    assert!(
        error.to_string().contains("552") || error.is_permanent(),
        "{error}"
    );
    assert_eq!(get_json(&api(&server, "messages")).await["total"], 0);
    server.shutdown();
}

#[tokio::test]
async fn retention_limit_prunes_oldest() {
    let server = start_with(&["--max-messages", "2"]).await;
    let client = reqwest::Client::new();
    for n in 1..=3 {
        client
            .post(api(&server, "send"))
            .json(&json!({ "from": "a@acme.test", "to": ["b@example.com"], "subject": format!("Message {n}") }))
            .send()
            .await
            .unwrap();
    }
    let list = get_json(&api(&server, "messages")).await;
    assert_eq!(list["total"], 2);
    assert_eq!(list["messages"][0]["subject"], "Message 3");
    assert_eq!(list["messages"][1]["subject"], "Message 2");
    server.shutdown();
}

#[tokio::test]
async fn sendmail_command_submits_stdin() {
    let server = start().await;
    let mut child = Command::new(env!("CARGO_BIN_EXE_mailroom"))
        .args(["sendmail", "-t", "-i", "-S", &server.smtp_addr.to_string()])
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"From: cron@host.test\nTo: ops@example.com\nBcc: hidden@example.com\nSubject: Nightly report\n\nAll good.\n")
        .unwrap();
    let status = tokio::task::spawn_blocking(move || child.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(status.success());

    let detail = get_json(&api(&server, "messages/latest")).await;
    assert_eq!(detail["subject"], "Nightly report");
    assert_eq!(
        detail["envelope"]["rcpt_to"],
        json!(["ops@example.com", "hidden@example.com"])
    );
    assert_eq!(detail["bcc"][0]["address"], "hidden@example.com");
    let raw = reqwest::get(api(&server, "messages/latest/raw"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(!raw.contains("Bcc:"), "sendmail -t must strip Bcc headers");
    server.shutdown();
}

#[tokio::test]
async fn serves_ui_under_webroot() {
    let server = start_with(&["--webroot", "/mail"]).await;
    assert!(server.url().ends_with("/mail/"));
    let index = reqwest::get(server.url()).await.unwrap();
    assert_eq!(index.status(), 200);
    let info = get_json(&api(&server, "info")).await;
    assert_eq!(info["version"], env!("CARGO_PKG_VERSION"));
    let root = reqwest::get(format!("http://{}/api/v1/info", server.http_addr))
        .await
        .unwrap();
    assert_eq!(root.status(), 404);
    server.shutdown();
}
