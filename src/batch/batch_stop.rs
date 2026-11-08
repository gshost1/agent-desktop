//! Why a batch stopped before its last entry, and the one place that decides
//! which reason wins when several apply to the same entry.

use agent_desktop_core::ErrorCode;
use serde_json::{Value, json};

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
    /// Decides whether the batch stops after the completed entry at `index`.
    /// `failure` is the entry's error code when it failed, and
    /// `deadline_stops` is whether the batch deadline has expired with
    /// entries left. A failure that is the batch deadline itself reports the
    /// deadline, so the reason does not depend on `--stop-on-error`.
    pub(super) fn after_entry(
        index: usize,
        failure: Option<&str>,
        oversized: bool,
        deadline_stops: bool,
        stop_on_error: bool,
    ) -> Option<Self> {
        let timed_out = failure == Some(ErrorCode::Timeout.as_str());
        if oversized {
            Some(Self::OutputLimit { index })
        } else if deadline_stops && timed_out {
            Some(Self::DeadlineAfterEntry { index })
        } else if failure.is_some() && stop_on_error {
            Some(Self::StopOnError { index })
        } else if deadline_stops {
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
