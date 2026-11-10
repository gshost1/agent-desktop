use agent_desktop_core::{ActionResult, NotificationInfo};
use serde_json::{Map, Value};

pub(crate) const LIST_RESTORE: &str = "list_restore";

/// A mutation's result after the message list the session opened could not
/// be shown closed again.
///
/// The mutation was delivered, so its result stays the answer: an error in
/// its place would invite a retry that lands on whichever entry now holds the
/// same index. A result with room for evidence names the restore as
/// `list_restore: "unverified"`; one without keeps its shape unchanged.
pub(crate) trait RestoreReport: Sized {
    fn with_unverified_restore(self) -> Self {
        self
    }
}

impl RestoreReport for ActionResult {
    fn with_unverified_restore(mut self) -> Self {
        let details = with_entry(self.details.take(), LIST_RESTORE, "unverified");
        self.with_details(details)
    }
}

impl RestoreReport for NotificationInfo {}

impl RestoreReport for (Vec<NotificationInfo>, Vec<String>) {}

pub(crate) fn with_entry(details: Option<Value>, key: &str, value: &str) -> Value {
    let mut map = match details {
        Some(Value::Object(map)) => map,
        _ => Map::new(),
    };
    map.insert(key.to_owned(), Value::from(value));
    Value::Object(map)
}

#[cfg(test)]
#[path = "restore_report_tests.rs"]
mod tests;
