//! The pre-dispatch zone of an action. Nothing that runs before
//! `execute_verified_action`, from taking the interaction lease to the
//! `action.dispatch.start` trace record, sends the requested action, so a
//! refusal raised there is safe to retry. Each such step runs through [`run`],
//! which states that once instead of each step having to remember to mark its
//! own errors.

use crate::{AdapterError, ref_action::ResolvedRefAction};
use serde_json::{Value, json};

/// Runs a step that comes before the requested action is sent. A refusal from
/// it becomes `not_delivered`, keeping any stronger evidence it carries.
pub(crate) fn run<T>(step: impl FnOnce() -> Result<T, AdapterError>) -> Result<T, AdapterError> {
    step().map_err(AdapterError::unsent)
}

/// A refusal raised once a focus call ran: nothing of the action was sent,
/// but the call may have raised a window, so `preparatory_focus` records it
/// the way `preparatory_scroll` records a scroll recovery.
pub(crate) fn after_focus(error: AdapterError) -> AdapterError {
    with_detail(
        error.unsent(),
        "preparatory_focus",
        json!("may_have_moved_focus"),
    )
}

/// Records the dispatch start, the last step before the action is sent.
pub(crate) fn trace_dispatch_start(
    target: &ResolvedRefAction<'_>,
    request: &crate::ActionRequest,
) -> Result<(), AdapterError> {
    run(|| {
        target
            .context
            .trace_lazy(
                "action.dispatch.start",
                || json!({ "ref": target.ref_id, "action": request.action.name() }),
            )
            .map_err(crate::ref_action::into_adapter_error)
    })
}

/// Adds one entry to an error's details. Details that are not an object are
/// kept under `error_details`.
pub(crate) fn with_detail(mut error: AdapterError, key: &str, value: Value) -> AdapterError {
    let mut details = error.details.take().unwrap_or_else(|| json!({}));
    if !details.is_object() {
        details = json!({ "error_details": details });
    }
    if let Some(object) = details.as_object_mut() {
        object.insert(key.into(), value);
    }
    error.with_details(details)
}

#[cfg(test)]
#[path = "pre_dispatch_tests.rs"]
mod tests;
