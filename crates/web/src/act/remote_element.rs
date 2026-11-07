use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};
use serde_json::{Value, json};

use super::element_call::ElementCall;
use crate::cdp::protocol_error::is_protocol_error;
use crate::{CdpConnection, PageSession};

const OBJECT_GROUP: &str = "agent-desktop";

/// Resolves a backend node to a remote object id. A node the page no longer
/// holds is `STALE_REF`. `DOM.resolveNode` needs no `Runtime.enable`, so the
/// page never sees the runtime domain switched on.
pub(crate) fn resolve_object(
    connection: &mut CdpConnection,
    session: &PageSession,
    backend_node_id: i64,
    deadline: Deadline,
) -> Result<String, AdapterError> {
    let reply = connection
        .call(
            "DOM.resolveNode",
            json!({ "backendNodeId": backend_node_id, "objectGroup": OBJECT_GROUP }),
            Some(&session.session_id),
            deadline,
        )
        .map_err(|error| stale_if_rejected(error, session, backend_node_id))?;
    reply
        .pointer("/object/objectId")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| stale(session, backend_node_id, "the node resolved to no object"))
}

/// Proves the node is still in the page, failing with `STALE_REF` if not.
pub(crate) fn ensure_live(
    connection: &mut CdpConnection,
    session: &PageSession,
    backend_node_id: i64,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let object_id = resolve_object(connection, session, backend_node_id, deadline)?;
    release_object(connection, session, &object_id, deadline);
    Ok(())
}

/// Runs `call` with the node bound as `this` and returns its JSON result.
pub(crate) fn call_on_element(
    connection: &mut CdpConnection,
    session: &PageSession,
    backend_node_id: i64,
    call: ElementCall,
    deadline: Deadline,
) -> Result<Value, AdapterError> {
    let object_id = resolve_object(connection, session, backend_node_id, deadline)?;
    let arguments: Vec<Value> = call
        .arguments
        .into_iter()
        .map(|value| json!({ "value": value }))
        .collect();
    let reply = connection.call(
        "Runtime.callFunctionOn",
        json!({
            "objectId": object_id,
            "functionDeclaration": call.function,
            "arguments": arguments,
            "returnByValue": true,
            "awaitPromise": false,
        }),
        Some(&session.session_id),
        deadline,
    );
    release_object(connection, session, &object_id, deadline);
    let mut reply = reply?;
    if let Some(exception) = reply.get("exceptionDetails") {
        return Err(script_failed(exception));
    }
    Ok(reply
        .pointer_mut("/result/value")
        .map_or(Value::Null, Value::take))
}

fn release_object(
    connection: &mut CdpConnection,
    session: &PageSession,
    object_id: &str,
    deadline: Deadline,
) {
    let _ = connection.call(
        "Runtime.releaseObject",
        json!({ "objectId": object_id }),
        Some(&session.session_id),
        deadline,
    );
}

fn stale_if_rejected(error: AdapterError, session: &PageSession, backend: i64) -> AdapterError {
    if !is_protocol_error(&error) {
        return error;
    }
    let detail = error.platform_detail.clone().unwrap_or_default();
    stale(session, backend, &detail)
}

pub(crate) fn stale(session: &PageSession, backend: i64, detail: &str) -> AdapterError {
    AdapterError::stale_ref_because(format!(
        "DOM node {backend} no longer exists in page target {}",
        session.target_id
    ))
    .with_platform_detail(detail)
    .with_details(json!({ "kind": "cdp_node_gone", "backend_node_id": backend }))
}

fn script_failed(exception: &Value) -> AdapterError {
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

#[cfg(test)]
#[path = "remote_element_tests.rs"]
mod tests;
