//! SMTP: the protocol state machine, the TCP server around it, and TLS setup.

pub mod reply;
pub mod server;
pub mod session;
pub mod tls;

pub use server::SmtpServer;
pub use session::{AuthPolicy, Envelope, SessionConfig};
