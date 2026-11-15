use agent_desktop_core::{AdapterError, Deadline, Point};
use serde_json::json;

use super::press_point::press_point;
use super::remote_element::prove_document;
use super::scroll::scroll_node;
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
    click_times(connection, session, token, 1, deadline)
}

/// [`click`] repeated `count` times at one point, each press carrying its
/// running click count, so two presses make the page's `dblclick`.
pub(crate) fn click_times(
    connection: &mut CdpConnection,
    session: &PageSession,
    token: &ElementToken,
    count: u32,
    deadline: Deadline,
) -> Result<Point, AdapterError> {
    prove_document(connection, session, token, deadline)?;
    press_node(connection, session, token, count, deadline)
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
    press_node(connection, session, token, 1, deadline)
}

/// Scrolls the node into view, finds a visible point of it that hit-tests
/// to the node or one of its descendants, then sends `count` press and
/// release pairs there. A node another element covers everywhere it was
/// tried is refused before any input.
fn press_node(
    connection: &mut CdpConnection,
    session: &PageSession,
    token: &ElementToken,
    count: u32,
    deadline: Deadline,
) -> Result<Point, AdapterError> {
    scroll_node(connection, session, token, deadline)?;
    let point = press_point(connection, session, token, deadline)?;
    for click_count in 1..=count.max(1) {
        press_once(connection, session, &point, click_count, deadline)?;
    }
    Ok(point)
}

fn press_once(
    connection: &mut CdpConnection,
    session: &PageSession,
    point: &Point,
    click_count: u32,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    for (kind, buttons) in [("mousePressed", 1), ("mouseReleased", 0)] {
        connection.call(
            "Input.dispatchMouseEvent",
            json!({
                "type": kind,
                "x": point.x,
                "y": point.y,
                "button": "left",
                "buttons": buttons,
                "clickCount": click_count,
            }),
            Some(&session.session_id),
            deadline,
        )?;
    }
    Ok(())
}
