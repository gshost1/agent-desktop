//! Evidence an [`AdapterError`] carries about what ran before it was raised.
//! Everything before an action is sent, from the interaction lease to the
//! dispatch-start trace record, is refused with [`AdapterError::unsent`]; a
//! refusal raised once a focus call ran also records that call with
//! [`AdapterError::after_focus`].

use serde_json::{Value, json};

use crate::AdapterError;

impl AdapterError {
    /// Adds one entry to the details through [`Self::with_details`]. Details
    /// that are not an object are kept under `error_details`.
    pub(crate) fn with_detail(mut self, key: &str, value: Value) -> Self {
        let mut details = self.details.take().unwrap_or_else(|| json!({}));
        if !details.is_object() {
            details = json!({ "error_details": details });
        }
        if let Some(object) = details.as_object_mut() {
            object.insert(key.into(), value);
        }
        self.with_details(details)
    }

    /// A refusal raised by a focus call or right after it, before the
    /// action: nothing of the action was sent, and [`Self::after_focus_ran`]
    /// records the call.
    pub(crate) fn after_focus(self) -> Self {
        self.unsent().after_focus_ran()
    }

    /// Records that a focus call ran before this error, which may have raised
    /// a window, the way `preparatory_scroll` records a scroll recovery. The
    /// disposition is left as it is, because the error may come from after
    /// the action was sent.
    pub(crate) fn after_focus_ran(self) -> Self {
        self.with_detail("preparatory_focus", json!("may_have_moved_focus"))
    }

    /// Whether this error records a focus call that ran before it.
    pub fn moved_focus(&self) -> bool {
        self.has_detail("preparatory_focus")
    }

    /// Whether this error records a preparatory scroll that ran before it,
    /// which may have changed the view.
    pub fn changed_view(&self) -> bool {
        self.has_detail("preparatory_scroll")
    }

    fn has_detail(&self, key: &str) -> bool {
        self.details
            .as_ref()
            .is_some_and(|details| details.get(key).is_some())
    }
}

#[cfg(test)]
#[path = "adapter_error_evidence_tests.rs"]
mod tests;
