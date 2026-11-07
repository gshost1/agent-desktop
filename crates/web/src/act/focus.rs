use agent_desktop_core::{AdapterError, Deadline};
use serde_json::json;

use super::remote_element::ensure_live;
use crate::{CdpConnection, PageSession};

/// Moves the page's keyboard focus to the node. Only the page's own focus
/// changes; the browser window is not raised.
pub fn focus(
    connection: &mut CdpConnection,
    session: &PageSession,
    backend_node_id: i64,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    ensure_live(connection, session, backend_node_id, deadline)?;
    connection
        .call(
            "DOM.focus",
            json!({ "backendNodeId": backend_node_id }),
            Some(&session.session_id),
            deadline,
        )
        .map(|_| ())
}
