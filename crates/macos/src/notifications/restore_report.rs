use agent_desktop_core::{ActionResult, AdapterError, NotificationInfo};
use serde_json::{Map, Value};

const LIST_RESTORE: &str = "list_restore";

/// What a call's result becomes when the Notification Center the session
/// opened could not be closed again or the previously frontmost app could not
/// be restored.
///
/// A mutation was sent, so its result stays the answer: an error in its place
/// would carry the cleanup's own retry-safe disposition and invite a retry
/// that presses or dismisses whichever notification now holds the index. A
/// mutation result with room for evidence names the cleanup as
/// `list_restore: "unverified"`; one without keeps its shape. A read changed
/// nothing, so its cleanup failure stays the error and a retry is harmless.
pub(crate) trait RestoreReport: Sized {
    fn after_failed_restore(self, cleanup: AdapterError) -> Result<Self, AdapterError>;
}

impl RestoreReport for ActionResult {
    fn after_failed_restore(mut self, cleanup: AdapterError) -> Result<Self, AdapterError> {
        warn_kept(&cleanup);
        let details = with_entry(self.details.take(), LIST_RESTORE, "unverified");
        Ok(self.with_details(details))
    }
}

impl RestoreReport for NotificationInfo {
    fn after_failed_restore(self, cleanup: AdapterError) -> Result<Self, AdapterError> {
        warn_kept(&cleanup);
        Ok(self)
    }
}

impl RestoreReport for (Vec<NotificationInfo>, Vec<String>) {
    fn after_failed_restore(self, cleanup: AdapterError) -> Result<Self, AdapterError> {
        warn_kept(&cleanup);
        Ok(self)
    }
}

impl RestoreReport for Vec<NotificationInfo> {
    fn after_failed_restore(self, cleanup: AdapterError) -> Result<Self, AdapterError> {
        Err(cleanup)
    }
}

/// Names the unverified cleanup on a failed call's own error, which stays the
/// primary answer with its own disposition.
pub(crate) fn error_with_unverified_restore(mut error: AdapterError) -> AdapterError {
    let details = with_entry(error.details.take(), LIST_RESTORE, "unverified");
    error.with_details(details)
}

fn warn_kept(cleanup: &AdapterError) {
    tracing::warn!(error = %cleanup, "Notification Center cleanup failed after a delivered mutation; its result is kept");
}

fn with_entry(details: Option<Value>, key: &str, value: &str) -> Value {
    let mut map = match details {
        Some(Value::Object(map)) => map,
        Some(other) => Map::from_iter([("error_details".to_owned(), other)]),
        None => Map::new(),
    };
    map.insert(key.to_owned(), Value::from(value));
    Value::Object(map)
}

#[cfg(test)]
#[path = "restore_report_tests.rs"]
mod tests;
