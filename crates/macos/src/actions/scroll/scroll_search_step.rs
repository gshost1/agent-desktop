//! What one step of a search page did.

use agent_desktop_core::AdapterError;

/// What one paging step did. A step that fails while it reads, before it
/// sends anything, returns that error instead, so nothing is settled for it.
#[derive(Debug)]
pub(super) enum SearchStep {
    /// The step had nothing to send.
    Skipped,
    /// The step was sent and accepted.
    Sent,
    /// Sending failed, and the application may still have acted on it.
    Refused(AdapterError),
}

impl From<Result<bool, AdapterError>> for SearchStep {
    fn from(result: Result<bool, AdapterError>) -> Self {
        match result {
            Ok(true) => Self::Sent,
            Ok(false) => Self::Skipped,
            Err(error) => Self::Refused(error),
        }
    }
}
