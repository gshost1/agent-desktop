use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};
use serde_json::{Value, json};

use crate::transport::transport_error::is_timeout;
use crate::{CdpConnection, CdpEvent, PageSession};

/// Navigates the page and waits, bounded by `deadline`, for the new
/// document's load event. A same-document navigation returns at once.
pub fn navigate(
    connection: &mut CdpConnection,
    session: &PageSession,
    url: &str,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    enable_load_events(connection, session, deadline)?;
    let session_id = session.session_id.as_str();
    connection.discard_events(|event| is_load_event(event, session_id, None));
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
    connection
        .wait_for_event(
            |event| is_load_event(event, session_id, Some((frame_id, loader_id))),
            deadline,
        )
        .map(|_| ())
        .map_err(|error| load_timeout(error, url))
}

/// Waits, bounded by `deadline`, until the page's current document has
/// finished loading. A page that already finished returns at once.
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
    connection
        .wait_for_event(|event| is_load_event(event, session_id, None), deadline)
        .map(|_| ())
        .map_err(|error| load_timeout(error, "the current document"))
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

fn is_load_event(event: &CdpEvent, session_id: &str, document: Option<(&str, &str)>) -> bool {
    if event.session_id.as_deref() != Some(session_id) {
        return false;
    }
    match event.method.as_str() {
        "Page.loadEventFired" => true,
        "Page.lifecycleEvent" => {
            let field = |key: &str| event.params.get(key).and_then(Value::as_str);
            field("name") == Some("load")
                && document.is_none_or(|(frame_id, loader_id)| {
                    field("frameId") == Some(frame_id) && field("loaderId") == Some(loader_id)
                })
        }
        _ => false,
    }
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
