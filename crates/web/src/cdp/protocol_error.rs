use agent_desktop_core::{AdapterError, ErrorCode};
use serde_json::Value;

const METHOD_NOT_FOUND: i64 = -32601;
const PROTOCOL_ERROR_KIND: &str = "cdp_protocol_error";

/// Maps a CDP `error` object to an adapter error. The CDP message travels in
/// `platform_detail`; the method and numeric code travel in `details`.
pub(crate) fn protocol_error(method: &str, error: &Value) -> AdapterError {
    let cdp_code = error.get("code").and_then(Value::as_i64).unwrap_or(0);
    let cdp_message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("unspecified DevTools error");
    let code = if cdp_code == METHOD_NOT_FOUND {
        ErrorCode::ActionNotSupported
    } else {
        ErrorCode::ActionFailed
    };
    let mut detail = cdp_message.to_owned();
    if let Some(data) = error.get("data").and_then(Value::as_str) {
        detail.push_str(": ");
        detail.push_str(data);
    }
    AdapterError::new(code, format!("DevTools rejected '{method}'"))
        .with_platform_detail(detail)
        .with_details(serde_json::json!({
            "kind": PROTOCOL_ERROR_KIND,
            "method": method,
            "cdp_code": cdp_code,
        }))
}

/// Whether the error is the browser's own refusal of a request, as opposed to
/// a transport failure or a timeout.
pub(crate) fn is_protocol_error(error: &AdapterError) -> bool {
    error
        .details
        .as_ref()
        .and_then(|details| details.get("kind"))
        .and_then(Value::as_str)
        == Some(PROTOCOL_ERROR_KIND)
}
