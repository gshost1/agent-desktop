//! What a completed batch entry's result says about why the batch might stop.

use agent_desktop_core::ErrorCode;
use serde_json::Value;

/// The facts about a completed entry that decide whether the batch stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct EntryOutcome {
    pub(super) index: usize,
    pub(super) failed: bool,
    pub(super) deadline_timeout: bool,
    pub(super) oversized: bool,
}

impl EntryOutcome {
    /// Reads the outcome of the entry at `index`. `batch_deadline_expired` is
    /// whether, while the entry ran, its code found a deadline expired that
    /// the batch deadline had capped (`Deadline::take_inherited_expiry`). A
    /// `TIMEOUT` with that record is the batch deadline's, whichever step of
    /// the entry ran out.
    pub(super) fn of(index: usize, entry: &Value, batch_deadline_expired: bool) -> Self {
        let failed = entry["ok"] != true;
        Self {
            index,
            failed,
            deadline_timeout: failed
                && batch_deadline_expired
                && entry["error"]["code"] == ErrorCode::Timeout.as_str(),
            oversized: false,
        }
    }

    pub(super) fn with_oversized(self, oversized: bool) -> Self {
        Self { oversized, ..self }
    }
}

#[cfg(test)]
#[path = "entry_outcome_tests.rs"]
mod tests;
