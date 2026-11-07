//! The pre-dispatch zone of a ref action. Nothing a ref action does before
//! `execute_verified_action`, from taking the interaction lease to the
//! `action.dispatch.start` trace record, sends the requested action, so a
//! refusal raised there is safe to retry. Each such step runs through
//! [`PreDispatch::run`], which states that once instead of each step having to
//! remember to mark its own errors.

use crate::{AdapterError, ref_action::ResolvedRefAction};
use serde_json::json;

/// A step of a ref action that runs before the requested action is sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PreDispatch {
    /// Taking the interaction lease, and confirming the budget outlived the
    /// wait for it.
    Lease,
    /// Raising the target application or window for headed input.
    Focus,
    /// Recording the dispatch start in a strict trace.
    Trace,
}

impl PreDispatch {
    /// Runs the step and marks any refusal from it with [`Self::refused`].
    pub(crate) fn run<T>(
        self,
        step: impl FnOnce() -> Result<T, AdapterError>,
    ) -> Result<T, AdapterError> {
        step().map_err(|error| self.refused(error))
    }

    /// Marks a refusal from this step `not_delivered`, keeping any stronger
    /// evidence the error already carries. A step with a side effect of its
    /// own records it in the details, as `preparatory_scroll` does for the
    /// scroll recovery, because the refused action left that change behind.
    pub(crate) fn refused(self, error: AdapterError) -> AdapterError {
        let error = error.unsent();
        match self.side_effect() {
            Some((key, value)) => with_detail(error, key, value),
            None => error,
        }
    }

    fn side_effect(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::Focus => Some(("preparatory_focus", "may_have_moved_focus")),
            Self::Lease | Self::Trace => None,
        }
    }
}

/// Records the dispatch start, the last step before the action is sent.
pub(crate) fn trace_dispatch_start(
    target: &ResolvedRefAction<'_>,
    request: &crate::ActionRequest,
) -> Result<(), AdapterError> {
    PreDispatch::Trace.run(|| {
        target
            .context
            .trace_lazy(
                "action.dispatch.start",
                || json!({ "ref": target.ref_id, "action": request.action.name() }),
            )
            .map_err(crate::ref_action::into_adapter_error)
    })
}

fn with_detail(mut error: AdapterError, key: &str, value: &str) -> AdapterError {
    let details = error.details.get_or_insert_with(|| json!({}));
    if !details.is_object() {
        *details = json!({ "error_details": details.take() });
    }
    if let Some(object) = details.as_object_mut() {
        object.insert(key.into(), value.into());
    }
    error
}

#[cfg(test)]
#[path = "pre_dispatch_tests.rs"]
mod tests;
