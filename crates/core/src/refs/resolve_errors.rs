use serde_json::json;

use crate::{
    AdapterError, CaptureVerdict, Deadline, ErrorCode, PlatformAdapter, RefEntry,
    system::process_state::ProcessState,
};

/// The incomplete-and-retryable answer shared by both resolvers: a candidate
/// that could not be read is not a non-match. `RefEntry` evidence is
/// platform-neutral, and an unreadable candidate means the same thing
/// whichever adapter's search produced it, so the payload - message,
/// suggestion, and the `complete: false, retryable: true` stamp - is decided
/// once, here, instead of by two copies that can drift apart.
pub fn identity_unknown_error(entry: &RefEntry) -> AdapterError {
    AdapterError::new(
        ErrorCode::Timeout,
        "Strict resolution could not determine candidate identity from the live accessibility evidence",
    )
    .with_suggestion("Retry after the target application finishes updating its accessibility tree")
    .with_disposition(crate::DeliverySemantics::not_delivered())
    .with_details(json!({
        "kind": "resolution_identity_unknown",
        "role": entry.identity.role,
        "complete": false,
        "retryable": true,
    }))
}

/// The refusal a tokenless ref gets from its capture verdict before anything
/// is read from the application, or `None` when the capture proved it unique.
fn capture_refusal(entry: &RefEntry) -> Option<AdapterError> {
    match entry.identity.capture {
        CaptureVerdict::Unique => None,
        CaptureVerdict::Ambiguous => Some(identity_not_unique(entry)),
        CaptureVerdict::Incomplete => Some(capture_incomplete(entry)),
        CaptureVerdict::Unknown => Some(capture_unknown(entry)),
    }
}

/// The refusal a saved ref gets before anything is read from the application:
/// a ref carrying an element token is judged by the adapter, every other ref by
/// its capture verdict.
pub(crate) fn refused_before_reading(entry: &RefEntry) -> Option<AdapterError> {
    match crate::refs::ref_identity::element_token(entry) {
        Some(_) => None,
        None => capture_refusal(entry),
    }
}

/// A tokenless ref whose capture found another element matching it, or whose
/// refmap predates the verdict and recorded only that it was not unique.
fn identity_not_unique(entry: &RefEntry) -> AdapterError {
    AdapterError::new(
        ErrorCode::AmbiguousTarget,
        "Saved ref was not proven unique when it was captured, so it cannot be re-identified safely",
    )
    .with_suggestion(
        "Another element in the window matched it when it was captured; a refmap from an \
         earlier version does not record why, so take a new snapshot for one. Identical twins \
         are refused again by every snapshot; where headed pointer input is available, an \
         explicit --headed coordinate click can pick one.",
    )
    .with_details(json!({
        "kind": "identity_not_unique",
        "retryable": false,
        "role": entry.identity.role,
    }))
    .with_disposition(crate::DeliverySemantics::not_delivered())
}

/// A tokenless ref whose capture was not a complete read of the whole window
/// and saw no twin, so it is stale rather than a twin. It is stamped not
/// retryable: repeating the ref returns the same refusal, and the generic
/// fresh-snapshot recovery would retake the snapshot at the timeout that just
/// fell short, so the suggestion asks for a larger one instead.
fn capture_incomplete(entry: &RefEntry) -> AdapterError {
    AdapterError::stale_ref_because(
        "Saved ref was not proven unique when it was captured, because the capture did not \
         read the whole window",
    )
    .with_suggestion(
        "Retake the snapshot with a larger --timeout-ms, so it can read the whole window, \
         then retry with the new ref.",
    )
    .with_details(json!({
        "kind": "identity_unproven",
        "reason": "capture_incomplete",
        "retryable": false,
        "role": entry.identity.role,
    }))
}

/// A tokenless ref whose identity no capture proved: an element in its window
/// left a field the ref records unread, the ref could not be tied to the
/// element it was built from, or no capture judged it at all. A new snapshot
/// may read the same way, so it is stamped not retryable and advertises no
/// fresh-snapshot recovery.
fn capture_unknown(entry: &RefEntry) -> AdapterError {
    AdapterError::stale_ref_because(
        "Saved ref's identity was not proven by any capture: an element in its window could \
         not be read, or no capture judged this ref",
    )
    .with_suggestion(
        "The element's identity cannot be read reliably, so a new snapshot may refuse it again. \
         Target it through a different selector, or, where headed pointer input is available, \
         an explicit --headed coordinate click.",
    )
    .with_details(json!({
        "kind": "identity_unproven",
        "reason": "capture_unknown",
        "retryable": false,
        "role": entry.identity.role,
    }))
}

/// Stamps the final incomplete diagnosis with `deadline_elapsed` so the
/// caller sees why the retries ran out, preserving the incomplete's own
/// details rather than discarding them for a bare `TIMEOUT`. Shared by both
/// resolvers' retry loops, which reach the same terminal shape when their own
/// deadline expires with an incomplete answer still on hand.
pub fn mark_deadline_elapsed(mut error: AdapterError) -> AdapterError {
    let mut details = error.details.take().unwrap_or_else(|| json!({}));
    if let Some(object) = details.as_object_mut() {
        object.insert("deadline_elapsed".into(), json!(true));
    } else {
        details = json!({
            "evidence": details,
            "deadline_elapsed": true,
        });
    }
    error.with_details(details)
}

pub(crate) fn ambiguous_live_match(entry: &RefEntry) -> AdapterError {
    AdapterError::ambiguous_target(
        "Two or more live elements match the saved ref's recorded role and stable identity",
    )
    .with_suggestion(
        "An element identical to the captured one appeared, so neither can be picked by ref and \
         a new snapshot refuses both as twins. Act on a control that is unique in the window, or, \
         where headed pointer input is available, use an explicit --headed coordinate click.",
    )
    .with_details(json!({
        "kind": "saved_ref_ambiguous",
        "retryable": false,
        "complete": true,
        "role": entry.identity.role,
    }))
}

pub(crate) fn absent(entry: &RefEntry) -> AdapterError {
    AdapterError::stale_ref_because("No live element matches the saved ref's recorded identity")
        .with_details(json!({
            "kind": "saved_ref_absent",
            "retryable": false,
            "complete": true,
            "role": entry.identity.role,
        }))
}

/// What a ref of an exited application should do next: the application may
/// run again under a new process, and only a new snapshot of it can mint refs.
pub(crate) const EXITED_PROCESS_SUGGESTION: &str = "The target process exited. Run 'list-apps' to see \
     whether the application runs again, launch it if it does not, then take a new snapshot";

/// A lookup that found no running application for a ref's recorded owner
/// answers as a stale ref only when the recorded process is gone, because no
/// live element can match the ref then. An application that still runs but
/// was not found by name, and every other failure, are returned as they are.
pub(crate) fn owner_gone(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    error: AdapterError,
    deadline: Deadline,
) -> AdapterError {
    if error.code != ErrorCode::AppNotFound {
        return error;
    }
    let Some(process) = entry.process.identity() else {
        return error;
    };
    match adapter.process_state(process, deadline) {
        Ok(state @ (ProcessState::Exited { .. } | ProcessState::Crashed { .. })) => {
            owner_exited(entry, &state)
        }
        _ => error,
    }
}

fn owner_exited(entry: &RefEntry, state: &ProcessState) -> AdapterError {
    AdapterError::stale_ref_because("The application the ref was taken in is no longer running")
        .with_suggestion(EXITED_PROCESS_SUGGESTION)
        .with_details(json!({
            "kind": "saved_ref_absent",
            "process_state": state.label(),
            "retryable": false,
            "complete": true,
            "role": entry.identity.role,
        }))
}

/// The refusal for a landed element that does not match the ref. After a
/// whole-window count found the ref's element just before the landing
/// (`counted`), the path moved under the descent; with no count behind the
/// landing, as in the C find entry point's first-match landing, nothing shows
/// the element still exists, so it is absent.
pub(crate) fn landed_elsewhere(entry: &RefEntry, counted: bool) -> AdapterError {
    if counted {
        landing_moved(entry)
    } else {
        absent(entry)
    }
}

/// The stored path landed an element that does not match the ref, right after
/// the whole-window count found the ref's element exactly once. The element
/// existed a moment earlier, so this is a path that moved under the descent
/// (a list re-sorting, rows refreshing), not proof the element is gone. Only
/// the descent disagreed with the count, so the evidence is incomplete and
/// retryable, and the next attempt counts again.
pub(crate) fn landing_moved(entry: &RefEntry) -> AdapterError {
    AdapterError::stale_ref_because(
        "The saved ref's element was found, but a different element took its place before it could be reached",
    )
    .with_suggestion(
        "The window was changing while the ref was resolved. Retry; if it keeps changing, \
         take a fresh snapshot once it settles.",
    )
    .with_details(json!({
        "kind": "saved_ref_landing_moved",
        "retryable": true,
        "complete": false,
        "role": entry.identity.role,
    }))
}

#[cfg(test)]
#[path = "resolve_errors_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "resolve_errors_owner_tests.rs"]
mod owner_tests;
