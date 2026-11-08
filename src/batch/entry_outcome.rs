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
    /// Reads the outcome of the entry at `index`. `batch_expired` is whether
    /// the batch deadline had expired when the entry returned. A `TIMEOUT`
    /// then is the batch deadline's: every entry runs under it, so nothing
    /// after the entry could have run either.
    pub(super) fn of(index: usize, entry: &Value, batch_expired: bool) -> Self {
        let failed = entry["ok"] != true;
        Self {
            index,
            failed,
            deadline_timeout: failed
                && batch_expired
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
