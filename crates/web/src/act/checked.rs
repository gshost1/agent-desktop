use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};
use serde_json::json;

use super::click::click;
use super::read::read_element;
use crate::{CdpConnection, PageSession};

/// Sets a checkbox, radio button or switch to `checked` by clicking it when
/// its state differs, then re-reads the state to confirm. Returns the final
/// state. A node with no checked state is `ACTION_NOT_SUPPORTED`; a click
/// that does not reach the wanted state (unchecking a radio button) is
/// `ACTION_FAILED`.
pub fn set_checked(
    connection: &mut CdpConnection,
    session: &PageSession,
    backend_node_id: i64,
    checked: bool,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let current = read_element(connection, session, backend_node_id, deadline)?
        .checked
        .ok_or_else(|| not_checkable(backend_node_id))?;
    if current == checked {
        return Ok(current);
    }
    click(connection, session, backend_node_id, deadline)?;
    let after = read_element(connection, session, backend_node_id, deadline)?.checked;
    if after == Some(checked) {
        return Ok(checked);
    }
    Err(AdapterError::new(
        ErrorCode::ActionFailed,
        format!(
            "DOM node {backend_node_id} did not become {}",
            if checked { "checked" } else { "unchecked" }
        ),
    )
    .with_details(json!({ "kind": "cdp_checked_unchanged", "observed": after }))
    .with_disposition(DeliverySemantics::delivered_unverified()))
}

fn not_checkable(backend_node_id: i64) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionNotSupported,
        format!("DOM node {backend_node_id} has no checked state"),
    )
    .with_disposition(DeliverySemantics::not_delivered())
}

#[cfg(test)]
#[path = "checked_tests.rs"]
mod tests;
