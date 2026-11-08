use serde_json::json;

use crate::{AdapterError, CaptureVerdict, ErrorCode, RefEntry};

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
pub(crate) fn capture_refusal(entry: &RefEntry) -> Option<AdapterError> {
    match entry.identity.capture {
        CaptureVerdict::Unique => None,
        CaptureVerdict::Ambiguous => Some(identity_not_unique(entry)),
        CaptureVerdict::Incomplete => Some(capture_incomplete(entry)),
        CaptureVerdict::Unknown => Some(capture_unknown(entry)),
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
/// and saw no twin. A snapshot that reads the whole window can prove it, so
/// this is a stale ref with the fresh-snapshot recovery rather than a twin. It
/// carries no `retryable` stamp, because repeating the same ref returns the
/// same refusal while a new snapshot can succeed.
fn capture_incomplete(entry: &RefEntry) -> AdapterError {
    AdapterError::stale_ref_because(
        "Saved ref was not proven unique when it was captured, because the capture did not \
         read the whole window",
    )
    .with_suggestion(
        "Retake the snapshot, with a larger --timeout-ms for a large window, then retry with \
         the new ref.",
    )
    .with_details(json!({
        "kind": "identity_unproven",
        "reason": "capture_incomplete",
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

#[cfg(test)]
#[path = "resolve_errors_tests.rs"]
mod tests;
