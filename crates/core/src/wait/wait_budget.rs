use std::time::Duration;

use serde_json::json;

use crate::{AdapterError, ErrorCode};

pub const MAX_WAIT_TIMEOUT_MS: u64 = 24 * 60 * 60 * 1000;

/// The default budget of `--wait-for` and `--wait-for-gone`. A post-action
/// wait that succeeds sees its effect within a few seconds of the action, so
/// a longer default only lengthens the waits whose effect never comes.
pub const DEFAULT_POST_ACTION_WAIT_MS: u64 = 10_000;

pub fn wait_timeout_duration(timeout_ms: u64) -> Result<Duration, AdapterError> {
    if timeout_ms > MAX_WAIT_TIMEOUT_MS {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            format!("Timeout exceeds the {MAX_WAIT_TIMEOUT_MS}ms maximum"),
        )
        .with_suggestion(format!(
            "Choose a timeout between 0 and {MAX_WAIT_TIMEOUT_MS} milliseconds"
        ))
        .with_details(json!({
            "timeout_ms": timeout_ms,
            "max_timeout_ms": MAX_WAIT_TIMEOUT_MS,
        })));
    }
    Ok(Duration::from_millis(timeout_ms))
}

#[cfg(test)]
#[path = "wait_budget_tests.rs"]
mod tests;
