use agent_desktop_core::{AdapterError, Deadline, Point};
use serde_json::json;

use super::remote_element::prove_document;
use super::scroll::{node_center, scroll_node};
use crate::{CdpConnection, ElementToken, PageSession};

/// Clicks the node's centre with synthesized page input. The events go to
/// the renderer, so the OS cursor never moves and the tab is never raised.
/// Only press and release are sent: a hidden background tab acknowledges a
/// `mouseMoved` only after a frame it never draws, which stalls the call for
/// five seconds, while the press already delivers the hover boundary events.
/// Returns the viewport point that was clicked.
pub fn click(
    connection: &mut CdpConnection,
    session: &PageSession,
    token: &ElementToken,
    deadline: Deadline,
) -> Result<Point, AdapterError> {
    prove_document(connection, session, token, deadline)?;
    click_node(connection, session, token, deadline)
}

/// Clicks a node whose document the caller already proved. The scroll
/// reports a node that is gone as `STALE_REF`, so no separate liveness
/// check runs.
pub(crate) fn click_node(
    connection: &mut CdpConnection,
    session: &PageSession,
    token: &ElementToken,
    deadline: Deadline,
) -> Result<Point, AdapterError> {
    scroll_node(connection, session, token, deadline)?;
    let point = node_center(connection, session, token, deadline)?;
    for (kind, buttons) in [("mousePressed", 1), ("mouseReleased", 0)] {
        connection.call(
            "Input.dispatchMouseEvent",
            json!({
                "type": kind,
                "x": point.x,
                "y": point.y,
                "button": "left",
                "buttons": buttons,
                "clickCount": 1,
            }),
            Some(&session.session_id),
            deadline,
        )?;
    }
    Ok(point)
}
