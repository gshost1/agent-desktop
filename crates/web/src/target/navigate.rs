use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};
use serde_json::{Value, json};

use crate::transport::transport_error::{is_timeout, transport_failed};
use crate::tree::frame_tree::list_frames;
use crate::{CdpConnection, CdpEvent, PageSession};

/// Navigates the page and waits, bounded by `deadline`, for the new
/// document's load: the `load` lifecycle event carrying the frame and
/// loader `Page.navigate` returned. A load event of any earlier document,
/// buffered or late, never satisfies the wait. A same-document navigation
/// returns at once.
pub fn navigate(
    connection: &mut CdpConnection,
    session: &PageSession,
    url: &str,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    enable_load_events(connection, session, deadline)?;
    let session_id = session.session_id.as_str();
    connection.discard_events(|event| is_any_load(event, session_id));
    let result = connection.call(
        "Page.navigate",
        json!({ "url": url }),
        Some(session_id),
        deadline,
    )?;
    if let Some(error_text) = result
        .get("errorText")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
    {
        return Err(navigation_failed(url, error_text));
    }
    let Some(loader_id) = result.get("loaderId").and_then(Value::as_str) else {
        return Ok(());
    };
    let frame_id = result
        .get("frameId")
        .and_then(Value::as_str)
        .unwrap_or_default();
    wait_for_document_load(connection, session_id, (frame_id, loader_id), deadline)
        .map_err(|error| load_timeout(error, url))
}

/// Waits, bounded by `deadline`, until the page's current document has
/// finished loading. A page that already finished returns at once. The
/// wait is bound to the main frame's loader read after the ready-state
/// check, so a buffered load event of an earlier document cannot end it.
pub fn wait_for_load(
    connection: &mut CdpConnection,
    session: &PageSession,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    enable_load_events(connection, session, deadline)?;
    let session_id = session.session_id.as_str();
    let state = connection.call(
        "Runtime.evaluate",
        json!({ "expression": "document.readyState", "returnByValue": true }),
        Some(session_id),
        deadline,
    )?;
    if state.pointer("/result/value").and_then(Value::as_str) == Some("complete") {
        return Ok(());
    }
    let main = list_frames(connection, session, deadline)?
        .into_iter()
        .next()
        .ok_or_else(|| transport_failed("Page.getFrameTree", "the page has no main frame"))?;
    let document = (main.frame_id.as_str(), main.loader_id.as_str());
    connection.discard_events(|event| {
        is_any_load(event, session_id) && !is_document_load(event, session_id, document)
    });
    wait_for_document_load(connection, session_id, document, deadline)
        .map_err(|error| load_timeout(error, "the current document"))
}

fn wait_for_document_load(
    connection: &mut CdpConnection,
    session_id: &str,
    document: (&str, &str),
    deadline: Deadline,
) -> Result<(), AdapterError> {
    connection
        .wait_for_event(
            |event| is_document_load(event, session_id, document),
            deadline,
        )
        .map(|_| ())
}

fn enable_load_events(
    connection: &mut CdpConnection,
    session: &PageSession,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let session_id = Some(session.session_id.as_str());
    connection.call("Page.enable", json!({}), session_id, deadline)?;
    connection.call(
        "Page.setLifecycleEventsEnabled",
        json!({ "enabled": true }),
        session_id,
        deadline,
    )?;
    Ok(())
}

fn is_any_load(event: &CdpEvent, session_id: &str) -> bool {
    event.session_id.as_deref() == Some(session_id)
        && (event.method == "Page.loadEventFired"
            || (event.method == "Page.lifecycleEvent"
                && event.params.get("name").and_then(Value::as_str) == Some("load")))
}

fn is_document_load(
    event: &CdpEvent,
    session_id: &str,
    (frame_id, loader_id): (&str, &str),
) -> bool {
    let field = |key: &str| event.params.get(key).and_then(Value::as_str);
    event.session_id.as_deref() == Some(session_id)
        && event.method == "Page.lifecycleEvent"
        && field("name") == Some("load")
        && field("frameId") == Some(frame_id)
        && field("loaderId") == Some(loader_id)
}

fn navigation_failed(url: &str, error_text: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        format!("Navigation to {url} failed"),
    )
    .with_platform_detail(error_text)
    .with_details(json!({ "kind": "cdp_navigation_failed", "url": url }))
    .with_disposition(DeliverySemantics::delivered_unverified())
}

fn load_timeout(error: AdapterError, what: &str) -> AdapterError {
    if !is_timeout(&error) {
        return error;
    }
    AdapterError::timeout(format!("{what} did not finish loading before the deadline"))
        .with_details(json!({ "kind": "cdp_load_timeout" }))
        .with_disposition(DeliverySemantics::delivered_unverified())
}

#[cfg(test)]
#[path = "navigate_tests.rs"]
mod tests;
