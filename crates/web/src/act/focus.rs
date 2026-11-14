use agent_desktop_core::{AdapterError, Deadline};
use serde_json::json;

use super::element_errors::node_error;
use super::remote_element::prove_document;
use crate::{CdpConnection, ElementToken, PageSession};

/// Moves the page's keyboard focus to the node. Only the page's own focus
/// changes; the browser window is not raised.
pub(crate) fn focus(
    connection: &mut CdpConnection,
    session: &PageSession,
    token: &ElementToken,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    prove_document(connection, session, token, deadline)?;
    connection
        .call(
            "DOM.focus",
            json!({ "backendNodeId": token.backend_node_id }),
            Some(&session.session_id),
            deadline,
        )
        .map(|_| ())
        .map_err(|error| node_error(error, token))
}
