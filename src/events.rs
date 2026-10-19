//! Change notifications, fanned out to SSE clients and long-polling waiters.

use serde_json::{Value, json};

use crate::{email::MessageSummary, storage::Stats};

#[derive(Debug, Clone)]
pub enum Event {
    /// A message arrived.
    Created(Box<MessageSummary>),
    /// Messages changed read state.
    Updated { ids: Vec<String>, read: bool },
    /// Messages were deleted (including pruning of old messages).
    Deleted { ids: Vec<String> },
    /// Every message was deleted.
    Cleared,
    /// Mailbox counters after a change.
    Stats(Stats),
}

impl Event {
    /// The SSE event name.
    pub fn name(&self) -> &'static str {
        match self {
            Event::Created(_) => "message.created",
            Event::Updated { .. } => "message.updated",
            Event::Deleted { .. } => "message.deleted",
            Event::Cleared => "messages.cleared",
            Event::Stats(_) => "stats",
        }
    }

    pub fn payload(&self) -> Value {
        match self {
            Event::Created(summary) => json!(summary),
            Event::Updated { ids, read } => json!({ "ids": ids, "read": read }),
            Event::Deleted { ids } => json!({ "ids": ids }),
            Event::Cleared => json!({}),
            Event::Stats(stats) => json!(stats),
        }
    }
}
