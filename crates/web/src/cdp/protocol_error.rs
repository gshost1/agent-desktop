use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode};
use serde_json::{Value, json};

const METHOD_NOT_FOUND: i64 = -32601;
const SESSION_NOT_FOUND: i64 = -32001;
const PROTOCOL_ERROR_KIND: &str = "cdp_protocol_error";
const TARGET_GONE_KIND: &str = "cdp_target_gone";
const TARGET_GONE_MESSAGES: [&str; 4] = [
    "Session with given id not found",
    "No session with given id",
    "No target with given id",
    "Target closed",
];
const NODE_GONE_MESSAGES: [&str; 6] = [
    "No node with given id",
    "Could not find node with given id",
    "No node found",
    "does not belong to the document",
    "Node is detached from document",
    "Node with given id does not exist",
];

/// Maps a CDP `error` object to an adapter error. The CDP message travels in
/// `platform_detail`; the method and numeric code travel in `details`. A
/// reply saying the flattened session or its target no longer exists is
/// `WINDOW_NOT_FOUND` with kind `cdp_target_gone`: the page itself is gone,
/// so a fresh snapshot of it cannot help.
pub(crate) fn protocol_error(method: &str, error: &Value) -> AdapterError {
    let cdp_code = error.get("code").and_then(Value::as_i64).unwrap_or(0);
    let cdp_message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("unspecified DevTools error");
    let mut detail = cdp_message.to_owned();
    if let Some(data) = error.get("data").and_then(Value::as_str) {
        detail.push_str(": ");
        detail.push_str(data);
    }
    if cdp_code == SESSION_NOT_FOUND
        || TARGET_GONE_MESSAGES
            .iter()
            .any(|message| detail.contains(message))
    {
        return target_gone(method, detail);
    }
    let code = if cdp_code == METHOD_NOT_FOUND {
        ErrorCode::ActionNotSupported
    } else {
        ErrorCode::ActionFailed
    };
    AdapterError::new(code, format!("DevTools rejected '{method}'"))
        .with_platform_detail(detail)
        .with_details(json!({
            "kind": PROTOCOL_ERROR_KIND,
            "method": method,
            "cdp_code": cdp_code,
        }))
}

/// The error for a request whose flattened session or target is gone,
/// whether the browser said so in a reply or announced the detach.
pub(crate) fn target_gone(method: &str, detail: impl Into<String>) -> AdapterError {
    AdapterError::new(
        ErrorCode::WindowNotFound,
        format!("The browser target behind '{method}' is gone"),
    )
    .with_suggestion("List the browser's pages again; the tab or frame was closed or detached")
    .with_platform_detail(detail)
    .with_details(json!({ "kind": TARGET_GONE_KIND, "method": method }))
    .with_disposition(DeliverySemantics::not_delivered())
}

/// Whether the error is the browser's own refusal of a request, as opposed to
/// a transport failure, a timeout or a target that is gone.
pub(crate) fn is_protocol_error(error: &AdapterError) -> bool {
    error_kind(error) == Some(PROTOCOL_ERROR_KIND)
}

/// Whether the browser reported the request's session or target as gone.
pub(crate) fn is_target_gone(error: &AdapterError) -> bool {
    error_kind(error) == Some(TARGET_GONE_KIND)
}

/// Whether the browser refused the request because the addressed DOM node
/// no longer exists. Only this refusal means a ref is stale.
pub(crate) fn is_node_gone(error: &AdapterError) -> bool {
    is_protocol_error(error)
        && error.platform_detail.as_deref().is_some_and(|detail| {
            NODE_GONE_MESSAGES
                .iter()
                .any(|message| detail.contains(message))
        })
}

fn error_kind(error: &AdapterError) -> Option<&str> {
    error
        .details
        .as_ref()
        .and_then(|details| details.get("kind"))
        .and_then(Value::as_str)
}

#[cfg(test)]
#[path = "protocol_error_tests.rs"]
mod tests;
