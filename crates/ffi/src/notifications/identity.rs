use crate::convert::string::optional_adapter_string;
use agent_desktop_core::{AdapterError, NotificationIdentity, NotificationTarget};

/// The C ABI addresses a notification by index and expected fields; core's
/// addressing rule validates them and checks them against a fresh read under
/// the mutation.
pub(crate) fn decode(
    index: usize,
    expected_app: *const std::os::raw::c_char,
    expected_title: *const std::os::raw::c_char,
) -> Result<NotificationTarget, AdapterError> {
    let identity = NotificationIdentity {
        expected_app: optional_adapter_string(expected_app, "expected_app")?,
        expected_title: optional_adapter_string(expected_title, "expected_title")?,
    };
    NotificationTarget::new(Some(index), None, identity)
}
