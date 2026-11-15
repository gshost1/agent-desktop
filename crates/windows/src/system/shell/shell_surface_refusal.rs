//! The errors a shell-surface open or close ends on once it has waited. A
//! raise or a dismissal keystroke that was already sent makes the error
//! `delivered_unverified`: the shell can apply it after the wait, and the
//! surface's accelerator toggles, so a second one can undo the first.

use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode};

use super::shell_surface::{SurfaceKindRow, kebab};

const READ_BEFORE_RAISING: &str = "Run 'list-surfaces' to see whether the surface is up before you open or close it again: the shell can apply an accelerator or an invoke after the deadline, and a second one can undo it";

/// The answer for a surface that is up but unreachable by keystroke, raised
/// before any keystroke was sent. It is not a timeout: waiting longer cannot
/// change it, because the surface never took the foreground the dismissal
/// needs. Naming the state lets a caller act on it instead of retrying.
#[cfg(target_os = "windows")]
pub(super) fn foreground_declined_error(row: &SurfaceKindRow) -> AdapterError {
    declined(row)
        .with_suggestion(
            "Activate the surface - click it, or raise it again from the shell - then retry the close",
        )
        .with_details(serde_json::json!({
            "kind": "shell_surface_declined_foreground",
            "retryable": true
        }))
        .with_disposition(DeliverySemantics::not_delivered())
}

/// The same state reached after the close re-ran the shell's raise: that
/// raise was sent, so the surface may still close or take the foreground.
#[cfg(target_os = "windows")]
pub(super) fn declined_after_raise_error(row: &SurfaceKindRow) -> AdapterError {
    declined(row)
        .with_suggestion(READ_BEFORE_RAISING)
        .with_details(serde_json::json!({ "kind": "shell_surface_declined_foreground" }))
        .with_disposition(DeliverySemantics::delivered_unverified())
}

#[cfg(target_os = "windows")]
fn declined(row: &SurfaceKindRow) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        format!(
            "the '{}' shell surface stayed up without taking the foreground, \
             so a dismissal keystroke cannot reach it",
            kebab(row.kind)
        ),
    )
}

/// An error raised after a raise or a dismissal keystroke went out: the input
/// landed, so a read that failed afterwards cannot say nothing changed.
pub(super) fn after_send(error: AdapterError) -> AdapterError {
    match error.disposition {
        DeliverySemantics::Unknown | DeliverySemantics::NotDelivered => error
            .with_disposition(DeliverySemantics::delivered_unverified())
            .with_suggestion(READ_BEFORE_RAISING),
        _ => error,
    }
}

/// The deadline passed while the surface was awaited. `sent` says whether a
/// raise or a dismissal keystroke went out first; a surface the taskbar
/// always presents is awaited without one, and its timeout is safe to repeat.
pub(super) fn timeout_error(
    row: &SurfaceKindRow,
    outcome: &str,
    kind: &'static str,
    sent: bool,
) -> AdapterError {
    let error = AdapterError::timeout(format!("the '{}' shell surface {outcome}", kebab(row.kind)));
    if sent {
        return error
            .with_suggestion(READ_BEFORE_RAISING)
            .with_details(serde_json::json!({ "kind": kind }))
            .with_disposition(DeliverySemantics::delivered_unverified());
    }
    error
        .with_suggestion(
            "Retry: nothing was sent, and the surface may still be loading; \
             a build that lacks the surface refuses instead of timing out",
        )
        .with_details(serde_json::json!({ "kind": kind, "retryable": true }))
        .with_disposition(DeliverySemantics::not_delivered())
}

#[cfg(test)]
#[path = "shell_surface_refusal_tests.rs"]
mod tests;
