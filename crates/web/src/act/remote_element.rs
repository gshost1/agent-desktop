use agent_desktop_core::{AdapterError, Deadline};
use serde_json::{Value, json};

use super::element_call::ElementCall;
use super::element_errors::{document_replaced, node_error, script_failed, stale, wrong_session};
use crate::tree::frame_tree::current_loader;
use crate::{CdpConnection, ElementToken, PageSession};

const OBJECT_GROUP: &str = "agent-desktop";

/// Proves the token still names a node of the document it was read from:
/// the session must belong to the token's target, and the token's frame
/// must still show the token's loader. A replaced document, or a frame that
/// is gone, is `STALE_REF`; backend node ids restart when a navigation swaps
/// the renderer, so without this proof an old ref could reach an unrelated
/// node.
pub(crate) fn prove_document(
    connection: &mut CdpConnection,
    session: &PageSession,
    token: &ElementToken,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    if session.target_id != token.target_id {
        return Err(wrong_session(token, &session.target_id));
    }
    let current = current_loader(connection, session, &token.frame_id, deadline)?;
    if current.as_deref() == Some(token.loader_id.as_str()) {
        return Ok(());
    }
    Err(document_replaced(token, current.as_deref()))
}

/// Proves the token's document, resolves its node to a remote object once,
/// runs `body` with the object id, and releases the object whatever `body`
/// returns. `DOM.resolveNode` needs no `Runtime.enable`, so the page never
/// sees the runtime domain switched on.
pub(crate) fn with_element<T>(
    connection: &mut CdpConnection,
    session: &PageSession,
    token: &ElementToken,
    deadline: Deadline,
    body: impl FnOnce(&mut CdpConnection, &str) -> Result<T, AdapterError>,
) -> Result<T, AdapterError> {
    prove_document(connection, session, token, deadline)?;
    let reply = connection
        .call(
            "DOM.resolveNode",
            json!({ "backendNodeId": token.backend_node_id, "objectGroup": OBJECT_GROUP }),
            Some(&session.session_id),
            deadline,
        )
        .map_err(|error| node_error(error, token))?;
    let object_id = reply
        .pointer("/object/objectId")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| stale(token, "the node resolved to no object"))?;
    let outcome = body(connection, &object_id);
    let _ = connection.call(
        "Runtime.releaseObject",
        json!({ "objectId": object_id }),
        Some(&session.session_id),
        deadline,
    );
    outcome
}

/// Runs `call` with the remote object bound as `this` and returns its JSON
/// result.
pub(crate) fn call_function(
    connection: &mut CdpConnection,
    session: &PageSession,
    object_id: &str,
    call: ElementCall,
    deadline: Deadline,
) -> Result<Value, AdapterError> {
    let arguments: Vec<Value> = call
        .arguments
        .into_iter()
        .map(|value| json!({ "value": value }))
        .collect();
    let mut reply = connection.call(
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
    )?;
    if let Some(exception) = reply.get("exceptionDetails") {
        return Err(script_failed(exception));
    }
    Ok(reply
        .pointer_mut("/result/value")
        .map_or(Value::Null, Value::take))
}

/// Runs `call` on the token's node after proving its document.
pub(crate) fn call_on_element(
    connection: &mut CdpConnection,
    session: &PageSession,
    token: &ElementToken,
    call: ElementCall,
    deadline: Deadline,
) -> Result<Value, AdapterError> {
    with_element(
        connection,
        session,
        token,
        deadline,
        |connection, object| call_function(connection, session, object, call, deadline),
    )
}

#[cfg(test)]
#[path = "remote_element_tests.rs"]
mod tests;
