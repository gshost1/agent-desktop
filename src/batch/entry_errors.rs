//! The errors a batch reports for entries it does not start.

use agent_desktop_core::{AdapterError, AppError, DeliverySemantics, ErrorCode};
use serde_json::json;

pub(super) fn baseline_error(
    blocked_index: usize,
    blocked_command: &str,
    wait_index: usize,
    wait_command: &str,
    mut source: AdapterError,
) -> AppError {
    let cause_details = source.details.take();
    source.message = format!(
        "Batch entry {blocked_index} ('{blocked_command}') was not started because the baseline for following wait entry {wait_index} ('{wait_command}') failed: {}",
        source.message
    );
    let mut details = json!({
        "kind": "pre_action_baseline_failed",
        "blocked_index": blocked_index,
        "blocked_command": blocked_command,
        "wait_index": wait_index,
        "wait_command": wait_command,
    });
    if let Some(cause_details) = cause_details {
        details["cause_details"] = cause_details;
    }
    source.details = Some(details);
    source.disposition = DeliverySemantics::not_delivered();
    source.into()
}

/// The error for a wait entry whose read, taken at an earlier entry so the
/// entries between could run, failed.
pub(super) fn wait_baseline_error(
    wait_index: usize,
    wait_command: &str,
    read_at: usize,
    read_command: &str,
    mut source: AdapterError,
) -> AppError {
    let cause_details = source.details.take();
    source.message = format!(
        "Batch entry {wait_index} ('{wait_command}') was not started because its baseline, read at entry {read_at} ('{read_command}'), failed: {}",
        source.message
    );
    let mut details = json!({
        "kind": "pre_action_baseline_failed",
        "wait_index": wait_index,
        "wait_command": wait_command,
        "read_at_index": read_at,
        "read_at_command": read_command,
    });
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
