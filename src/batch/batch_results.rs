//! The per-entry results a batch has collected so far.

use agent_desktop_core::AppError;
use serde_json::Value;

use super::{
    batch_stop::BatchStop, bounded_json::serialized_size, entry_errors::batch_timeout,
    result_entry::not_started_entry,
};

/// The results in entry order, with the bytes they take in the response and
/// how many entries completed.
pub(super) struct BatchResults {
    entries: Vec<Value>,
    bytes: usize,
    completed: usize,
}

impl BatchResults {
    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            bytes: 0,
            completed: 0,
        }
    }

    pub(super) fn bytes(&self) -> usize {
        self.bytes
    }

    pub(super) fn completed(&self) -> usize {
        self.completed
    }

    pub(super) fn into_entries(self) -> Vec<Value> {
        self.entries
    }

    pub(super) fn push(&mut self, entry: Value) {
        self.completed += usize::from(entry["execution"] == "completed");
        self.bytes = self
            .bytes
            .saturating_add(serialized_size(&entry).saturating_add(1));
        self.entries.push(entry);
    }

    pub(super) fn not_started(
        &mut self,
        index: usize,
        command: &str,
        reason: &str,
        error: AppError,
    ) {
        self.push(not_started_entry(index, command, reason, error));
    }

    /// Records the entry at `index` as not started because the batch deadline
    /// expired before it, and returns the stop that ends the batch there.
    pub(super) fn deadline(&mut self, index: usize, command: &str, timeout_ms: u64) -> BatchStop {
        self.not_started(
            index,
            command,
            "deadline",
            batch_timeout(index, command, timeout_ms),
        );
        BatchStop::DeadlineBeforeEntry { index }
    }
}
