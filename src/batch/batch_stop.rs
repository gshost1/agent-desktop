//! Why a batch stopped early, and the one place that decides
//! which reason wins when several apply to the same entry.

use serde_json::{Value, json};

use super::entry_outcome::EntryOutcome;

/// The reason a batch ended early, serialized as the body's `stopped`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum BatchStop {
    DeadlineBeforeEntry {
        index: usize,
    },
    DeadlineAfterEntry {
        index: usize,
    },
    StopOnError {
        index: usize,
    },
    OutputLimit {
        index: usize,
    },
    BaselineFailed {
        blocked_index: usize,
        blocked_command: String,
        wait_index: usize,
        wait_command: String,
    },
}

impl BatchStop {
    /// Decides whether the batch stops after the completed entry in
    /// `outcome`. `deadline_stops_rest` is whether the batch deadline has
    /// expired with entries left. A failure caused by the batch deadline
    /// reports the deadline, also on the last entry, so the reason does not
    /// depend on `--stop-on-error`.
    pub(super) fn after_entry(
        outcome: &EntryOutcome,
        deadline_stops_rest: bool,
        stop_on_error: bool,
    ) -> Option<Self> {
        let index = outcome.index;
        if outcome.oversized {
            Some(Self::OutputLimit { index })
        } else if outcome.deadline_timeout {
            Some(Self::DeadlineAfterEntry { index })
        } else if outcome.failed && stop_on_error {
            Some(Self::StopOnError { index })
        } else if deadline_stops_rest {
            Some(Self::DeadlineAfterEntry { index })
        } else {
            None
        }
    }

    pub(super) fn to_value(&self) -> Value {
        match self {
            Self::DeadlineBeforeEntry { index } => json!({ "reason": "deadline", "index": index }),
            Self::DeadlineAfterEntry { index } => {
                json!({ "reason": "deadline", "after_index": index })
            }
            Self::StopOnError { index } => json!({ "reason": "stop_on_error", "index": index }),
            Self::OutputLimit { index } => json!({ "reason": "output_limit", "index": index }),
            Self::BaselineFailed {
                blocked_index,
                blocked_command,
                wait_index,
                wait_command,
            } => json!({
                "reason": "pre_action_baseline_failed",
                "blocked_index": blocked_index,
                "blocked_command": blocked_command,
                "wait_index": wait_index,
                "wait_command": wait_command,
            }),
        }
    }
}

#[cfg(test)]
#[path = "batch_stop_tests.rs"]
mod tests;
