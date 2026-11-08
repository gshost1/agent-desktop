use agent_desktop_core::AdapterError;
use serde_json::{Map, Value};

const POSTED: &str = "emergency_release_posted";
const ACKNOWLEDGED: &str = "emergency_release_acknowledged";

/// What an aborted X11 sequence had to post to leave no key, button or keymap edit behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EmergencyRelease {
    /// Nothing was held or remapped when the sequence stopped, so no corrective input was posted.
    NotNeeded,
    /// Corrective input was posted; `acknowledged` is true only when the closing round trip came back.
    Posted { acknowledged: bool },
}

impl EmergencyRelease {
    /// Writes the release into an error's details under the shared envelope field names.
    pub(crate) fn record_into(self, details: &mut Map<String, Value>) {
        if let Self::Posted { acknowledged } = self {
            details.insert(POSTED.into(), true.into());
            details.insert(ACKNOWLEDGED.into(), acknowledged.into());
        }
    }

    /// Reads back the release an aborting worker recorded in its error.
    pub(crate) fn reported_by(error: &AdapterError) -> Self {
        let details = error.details.as_ref();
        if details.and_then(|map| map.get(POSTED)) != Some(&Value::Bool(true)) {
            return Self::NotNeeded;
        }
        Self::Posted {
            acknowledged: details.and_then(|map| map.get(ACKNOWLEDGED)) == Some(&Value::Bool(true)),
        }
    }
}
