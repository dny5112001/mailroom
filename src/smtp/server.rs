//! The network side of the SMTP server: accepts connections and drives a
//! [`Session`] for each one.

use std::{io, net::SocketAddr, sync::Arc, time::Duration};

use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    time::timeout,
};
use tokio_rustls::TlsAcceptor;
use tracing::{debug, error, info, warn};

use super::{
    reply::Reply,
    session::{Action, ChunkOutcome, Envelope, Session, SessionConfig},
};
use crate::Mailroom;

/// Longest accepted command line, including CRLF. RFC 5321 requires 512; real
/// clients sometimes send longer AUTH responses, so allow some headroom.
const MAX_COMMAND_LINE: usize = 4096;
/// Message content is read in slices of this size so a single enormous line
/// cannot exhaust memory.
const DATA_SLICE: u64 = 64 * 1024;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(300);
const DATA_TIMEOUT: Duration = Duration::from_secs(600);

trait Stream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Stream for T {}

type Connection = BufReader<Box<dyn Stream>>;

#[derive(Clone)]
pub struct SmtpServer {
    app: Mailroom,
    config: Arc<SessionConfig>,
    tls: Option<TlsAcceptor>,
}

impl SmtpServer {
    pub fn new(app: Mailroom, mut config: SessionConfig, tls: Option<TlsAcceptor>) -> Self {
        config.tls_available = tls.is_some();
        Self {
            app,
            config: Arc::new(config),
            tls,
        }
    }

    /// Accept connections forever.
    pub async fn serve(self, listener: TcpListener) {
        loop {
            let (stream, peer) = match listener.accept().await {
                Ok(accepted) => accepted,
                Err(err) => {
                    warn!("smtp accept failed: {err}");
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    continue;
                }
            };
            let server = self.clone();
            tokio::spawn(async move {
                debug!(%peer, "smtp connection opened");
                match server.handle(stream, peer).await {
                    Ok(()) => debug!(%peer, "smtp connection closed"),
                    Err(err) => debug!(%peer, "smtp connection ended: {err}"),
                }
            });
        }
    }

    async fn handle(&self, stream: TcpStream, peer: SocketAddr) -> io::Result<()> {
        let _ = stream.set_nodelay(true);
        let mut conn: Connection = BufReader::new(Box::new(stream));
        let mut session = Session::new(self.config.clone());
        let mut line = Vec::with_capacity(512);

        send(&mut conn, &session.greeting()).await?;

        loop {
            line.clear();
            let action = match timeout(COMMAND_TIMEOUT, read_line(&mut conn, &mut line)).await {
                Err(_) => {
                    send(&mut conn, &session.timeout_reply()).await?;
                    return Ok(());
                }
                Ok(result) => match result? {
                    LineRead::Eof => return Ok(()),
                    LineRead::TooLong => session.line_too_long(),
                    LineRead::Line => session.handle_line(&line),
                },
            };

            match action {
                Action::Reply(reply) => send(&mut conn, &reply).await?,
                Action::Close(reply) => {
                    send(&mut conn, &reply).await?;
                    let _ = conn.get_mut().shutdown().await;
                    return Ok(());
                }
                Action::ReadData(reply) => {
                    send(&mut conn, &reply).await?;
                    let limit = self.config.max_message_size;
                    let (data, too_large) =
                        match timeout(DATA_TIMEOUT, read_data(&mut conn, limit)).await {
                            Ok(result) => result?,
                            Err(_) => {
                                send(&mut conn, &session.timeout_reply()).await?;
                                return Ok(());
                            }
                        };
                    let reply = match session.handle_data(data, too_large) {
                        Ok(envelope) => self.deliver(envelope, peer).await,
                        Err(reply) => reply,
                    };
                    send(&mut conn, &reply).await?;
                }
                Action::ReadChunk { size, last } => {
                    let capacity = session.chunk_capacity();
                    let (chunk, overflow) =
                        match timeout(DATA_TIMEOUT, read_chunk(&mut conn, size, capacity)).await {
                            Ok(result) => result?,
                            Err(_) => {
                                send(&mut conn, &session.timeout_reply()).await?;
                                return Ok(());
                            }
                        };
                    let reply = match session.handle_chunk(chunk, last, overflow) {
                        ChunkOutcome::Reply(reply) => reply,
                        ChunkOutcome::Deliver(envelope) => self.deliver(envelope, peer).await,
                    };
                    send(&mut conn, &reply).await?;
                }
                Action::StartTls(reply) => {
                    let Some(acceptor) = &self.tls else {
                        send(&mut conn, &Reply::new(454, "4.7.0 TLS not available")).await?;
                        continue;
                    };
                    send(&mut conn, &reply).await?;
                    // Anything pipelined after STARTTLS is discarded with the old
                    // buffer, as required by RFC 3207 §5.
                    let stream = conn.into_inner();
                    let tls_stream = acceptor.accept(stream).await?;
                    conn = BufReader::new(Box::new(tls_stream));
                    session.tls_established();
                }
            }
        }
    }

    async fn deliver(&self, envelope: Envelope, peer: SocketAddr) -> Reply {
        match self.app.ingest_smtp(envelope, Some(peer)).await {
            Ok(summary) => {
                info!(
                    id = %summary.id,
                    from = %summary.from.as_ref().map(|a| a.address.as_str()).unwrap_or(""),
                    subject = %summary.subject,
                    size = summary.size,
                    "message received"
                );
                Reply::new(250, format!("2.0.0 Ok: queued as {}", summary.id))
            }
            Err(err) => {
                error!("failed to store message: {err:#}");
                Reply::new(451, "4.3.0 Error: could not store message")
            }
        }
    }
}

enum LineRead {
    Line,
    TooLong,
    Eof,
}

async fn send(conn: &mut Connection, reply: &Reply) -> io::Result<()> {
    let stream = conn.get_mut();
    stream.write_all(reply.to_wire().as_bytes()).await?;
    stream.flush().await
}

/// Read one command line. Over-long lines are consumed up to their end and
/// reported as [`LineRead::TooLong`].
async fn read_line(conn: &mut Connection, buf: &mut Vec<u8>) -> io::Result<LineRead> {
    let n = (&mut *conn)
        .take(MAX_COMMAND_LINE as u64)
        .read_until(b'\n', buf)
        .await?;
    if n == 0 {
        return Ok(LineRead::Eof);
    }
    if buf.ends_with(b"\n") {
        return Ok(LineRead::Line);
    }
    if n < MAX_COMMAND_LINE {
        // Connection closed in the middle of a line.
        return Ok(LineRead::Eof);
    }
    // Discard the rest of the over-long line.
    let mut discard = Vec::new();
    loop {
        discard.clear();
        let n = (&mut *conn)
            .take(DATA_SLICE)
            .read_until(b'\n', &mut discard)
            .await?;
        if n == 0 {
            return Ok(LineRead::Eof);
        }
        if discard.ends_with(b"\n") {
            return Ok(LineRead::TooLong);
        }
    }
}

/// Read DATA content up to the `.` terminator, undoing dot-stuffing. Content
/// beyond `limit` bytes is consumed but discarded and reported as too large.
async fn read_data(conn: &mut Connection, limit: usize) -> io::Result<(Vec<u8>, bool)> {
    let mut data = Vec::with_capacity(8 * 1024);
    let mut slice = Vec::with_capacity(1024);
    let mut at_line_start = true;
    let mut too_large = false;

    loop {
        slice.clear();
        let n = (&mut *conn)
            .take(DATA_SLICE)
            .read_until(b'\n', &mut slice)
            .await?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "connection closed during DATA",
            ));
        }

        let mut content = slice.as_slice();
        if at_line_start {
            if content == b".\r\n" || content == b".\n" {
                return Ok((data, too_large));
            }
            if content.first() == Some(&b'.') {
                content = &content[1..];
            }
        }
        at_line_start = slice.ends_with(b"\n");

        if !too_large {
            if data.len() + content.len() > limit {
                too_large = true;
                data = Vec::new();
            } else {
                data.extend_from_slice(content);
            }
        }
    }
}

/// Read a BDAT chunk of exactly `size` bytes. Chunks larger than `capacity`
/// are drained and reported as overflowing.
async fn read_chunk(
    conn: &mut Connection,
    size: usize,
    capacity: usize,
) -> io::Result<(Vec<u8>, bool)> {
    if size > capacity {
        let copied =
            tokio::io::copy(&mut (&mut *conn).take(size as u64), &mut tokio::io::sink()).await?;
        if copied < size as u64 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "connection closed during BDAT",
            ));
        }
        return Ok((Vec::new(), true));
    }
    let mut chunk = vec![0; size];
    conn.read_exact(&mut chunk).await?;
    Ok((chunk, false))
}
