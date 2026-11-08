use agent_desktop_core::{AdapterError, ErrorCode};

pub(crate) fn transport_timeout(operation: &str) -> AdapterError {
    AdapterError::timeout(format!(
        "The DevTools connection did not finish '{operation}' before the deadline"
    ))
    .with_details(serde_json::json!({ "kind": "cdp_timeout", "operation": operation }))
}

pub(crate) fn transport_closed(detail: impl Into<String>) -> AdapterError {
    AdapterError::new(ErrorCode::ActionFailed, "The DevTools connection closed")
        .with_suggestion("Reconnect to the browser; it may have exited or dropped the session")
        .with_platform_detail(detail)
        .with_details(serde_json::json!({ "kind": "cdp_transport_closed" }))
}

pub(crate) fn transport_failed(operation: &str, detail: impl Into<String>) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        format!("The DevTools connection failed during '{operation}'"),
    )
    .with_platform_detail(detail)
    .with_details(serde_json::json!({ "kind": "cdp_transport_failed", "operation": operation }))
}

pub(crate) fn is_timeout(error: &AdapterError) -> bool {
    error.code == ErrorCode::Timeout
}
