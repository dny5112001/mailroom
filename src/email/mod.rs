//! Email content: the internal model, MIME parsing, composition and
//! extraction of links and verification codes.

pub mod compose;
pub mod extract;
pub mod model;
pub mod parser;

pub use model::{Address, Envelope, MessageDetail, MessageSummary};
