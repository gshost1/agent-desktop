//! The errors a batch reports for entries it does not start.

use agent_desktop_core::{AdapterError, AppError, DeliverySemantics, ErrorCode};
use serde_json::json;

use super::preparation::PreparedCommand;

/// The error for an entry not started because the read for the wait at
/// `wait_index`, taken at entry `read_at`, failed. The entry not started is
/// either `read_at` itself, an action that must not run unobserved, or the
/// wait, when the read was held for it.
pub(super) fn baseline_error(
    commands: &[PreparedCommand],
    not_started: usize,
    read_at: usize,
    wait_index: usize,
    mut source: AdapterError,
) -> AppError {
    let read_command = commands[read_at].name.as_str();
    let wait_command = commands[wait_index].name.as_str();
    let cause_details = source.details.take();
    let (message, mut details) = if not_started == read_at {
        (
            format!(
                "Batch entry {read_at} ('{read_command}') was not started because the baseline for following wait entry {wait_index} ('{wait_command}') failed"
            ),
            json!({ "blocked_index": read_at, "blocked_command": read_command }),
        )
    } else {
        (
            format!(
                "Batch entry {wait_index} ('{wait_command}') was not started because its baseline, read at entry {read_at} ('{read_command}'), failed"
            ),
            json!({ "read_at_index": read_at, "read_at_command": read_command }),
        )
    };
    source.message = format!("{message}: {}", source.message);
    details["kind"] = json!("pre_action_baseline_failed");
    details["wait_index"] = json!(wait_index);
    details["wait_command"] = json!(wait_command);
    if let Some(cause_details) = cause_details {
        details["cause_details"] = cause_details;
    }
    source.details = Some(details);
    source.disposition = DeliverySemantics::not_delivered();
    source.into()
}

pub(super) fn batch_session_ended(index: usize, command: &str, session_id: &str) -> AppError {
    AdapterError::new(
        ErrorCode::InvalidArgs,
        format!(
            "Batch entry {index} ('{command}') was not started because session '{session_id}' has ended"
        ),
    )
    .with_suggestion("Re-run the entry outside the ended session or start a new session")
    .with_details(json!({
        "kind": "batch_session_ended",
        "batch_index": index,
        "batch_command": command,
        "session_id": session_id,
    }))
    .with_disposition(DeliverySemantics::not_delivered())
    .into()
}

pub(super) fn batch_timeout(index: usize, command: &str, timeout_ms: u64) -> AppError {
    AdapterError::timeout("Batch deadline elapsed before the entry started")
        .with_details(json!({
            "kind": "batch_deadline",
            "batch_index": index,
            "batch_command": command,
            "timeout_ms": timeout_ms,
        }))
        .with_disposition(DeliverySemantics::not_delivered())
        .into()
}
