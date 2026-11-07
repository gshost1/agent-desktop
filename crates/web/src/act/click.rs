use agent_desktop_core::{AdapterError, Deadline, Point};
use serde_json::json;

use super::remote_element::ensure_live;
use super::scroll::{box_center, scroll_into_view};
use crate::{CdpConnection, PageSession};

/// Clicks the node's centre with synthesized page input. The events go to
/// the renderer, so the OS cursor never moves and the tab is never raised.
/// Only press and release are sent: a hidden background tab acknowledges a
/// `mouseMoved` only after a frame it never draws, which stalls the call for
/// five seconds, while the press already delivers the hover boundary events.
/// Returns the viewport point that was clicked.
pub fn click(
    connection: &mut CdpConnection,
    session: &PageSession,
    backend_node_id: i64,
    deadline: Deadline,
) -> Result<Point, AdapterError> {
    ensure_live(connection, session, backend_node_id, deadline)?;
    scroll_into_view(connection, session, backend_node_id, deadline)?;
    let point = box_center(connection, session, backend_node_id, deadline)?;
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
