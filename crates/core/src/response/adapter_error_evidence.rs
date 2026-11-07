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

    /// A refusal raised once a focus call ran: nothing of the action was
    /// sent, but the call may have raised a window, so `preparatory_focus`
    /// records it the way `preparatory_scroll` records a scroll recovery.
    pub(crate) fn after_focus(self) -> Self {
        self.unsent()
            .with_detail("preparatory_focus", json!("may_have_moved_focus"))
    }

    /// Whether a focus call ran before this refusal. A wait does not go back
    /// for another attempt after one, so a command raises a window at most
    /// once.
    pub(crate) fn moved_focus(&self) -> bool {
        self.details
            .as_ref()
            .is_some_and(|details| details.get("preparatory_focus").is_some())
    }
}

#[cfg(test)]
#[path = "adapter_error_evidence_tests.rs"]
mod tests;
