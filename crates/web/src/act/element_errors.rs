use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode};
use serde_json::{Value, json};

use crate::ElementToken;
use crate::cdp::protocol_error::is_node_gone;

/// Turns the browser's "no such node" refusal into `STALE_REF` and passes
/// every other error through unchanged: a target that is gone stays
/// `WINDOW_NOT_FOUND`, and any other refusal stays `ACTION_FAILED`, so the
/// recovery is never "refresh the snapshot" for a failure a refresh cannot
/// fix.
pub(crate) fn node_error(error: AdapterError, token: &ElementToken) -> AdapterError {
    if !is_node_gone(&error) {
        return error;
    }
    let detail = error.platform_detail.clone().unwrap_or_default();
    stale(token, &detail)
}

pub(crate) fn stale(token: &ElementToken, detail: &str) -> AdapterError {
    AdapterError::stale_ref_because(format!(
        "DOM node {} no longer exists in target {}",
        token.backend_node_id, token.target_id
    ))
    .with_platform_detail(detail)
    .with_details(json!({ "kind": "cdp_node_gone", "backend_node_id": token.backend_node_id }))
}

pub(crate) fn document_replaced(token: &ElementToken, current: Option<&str>) -> AdapterError {
    AdapterError::stale_ref_because(format!(
        "Frame {} no longer shows the document DOM node {} was read from",
        token.frame_id, token.backend_node_id
    ))
    .with_suggestion("Take a new snapshot and use the ref it returns")
    .with_details(json!({
        "kind": "cdp_document_replaced",
        "frame_id": token.frame_id,
        "expected_loader_id": token.loader_id,
        "current_loader_id": current,
    }))
}

pub(crate) fn wrong_session(token: &ElementToken, session_target: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::InvalidArgs,
        format!(
            "Element token for target {} was used with a session attached to {session_target}",
            token.target_id
        ),
    )
    .with_suggestion("Attach to the target the element token names")
    .with_disposition(DeliverySemantics::not_delivered())
}

pub(crate) fn script_failed(exception: &Value) -> AdapterError {
    let text = exception
        .pointer("/exception/description")
        .or_else(|| exception.get("text"))
        .and_then(Value::as_str)
        .unwrap_or("script exception");
    AdapterError::new(
        ErrorCode::ActionFailed,
        "The element script threw an exception",
    )
    .with_platform_detail(text)
    .with_details(json!({ "kind": "cdp_script_exception" }))
    .with_disposition(DeliverySemantics::delivered_unverified())
}
