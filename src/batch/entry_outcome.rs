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
    /// Reads the outcome of the entry at `index`. The entry ran under the
    /// earlier of its own deadline and the batch deadline, which had
    /// `batch_remaining_ms` left when it was dispatched. Its `TIMEOUT` is the
    /// batch deadline's when the batch deadline has expired and the entry's
    /// own budget, when its error reports one, would have run past what the
    /// batch had left.
    pub(super) fn of(
        index: usize,
        entry: &Value,
        batch_remaining_ms: u64,
        batch_expired: bool,
    ) -> Self {
        let failed = entry["ok"] != true;
        let error = &entry["error"];
        let own_budget_outlasts_batch = error["details"]["timeout_ms"]
            .as_u64()
            .is_none_or(|own_ms| own_ms >= batch_remaining_ms);
        Self {
            index,
            failed,
            deadline_timeout: failed
                && batch_expired
                && error["code"] == ErrorCode::Timeout.as_str()
                && own_budget_outlasts_batch,
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
