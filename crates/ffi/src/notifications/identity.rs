use crate::convert::string::optional_adapter_string;
use agent_desktop_core::{AdapterError, ErrorCode, NotificationIdentity, NotificationTarget};

/// The C ABI addresses a notification by index and expected fields; core
/// checks them against a fresh read under the mutation.
pub(crate) fn decode(
    index: usize,
    expected_app: *const std::os::raw::c_char,
    expected_title: *const std::os::raw::c_char,
) -> Result<NotificationTarget, AdapterError> {
    let identity = NotificationIdentity {
        expected_app: optional_adapter_string(expected_app, "expected_app")?,
        expected_title: optional_adapter_string(expected_title, "expected_title")?,
    };
    if identity.is_empty() {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "expected_app or expected_title is required",
        ));
    }
    NotificationTarget::new(Some(index), None, identity)
}
