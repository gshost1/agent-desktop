use agent_desktop_core::{AdapterError, Deadline, Point};
use serde_json::{Value, json};

use super::hit_test::{confirm_hit, is_intercepted};
use super::scroll::{not_rendered, not_rendered_error, quad_center};
use super::viewport_metrics::ViewportMetrics;
use crate::{CdpConnection, ElementToken, PageSession};

const MAX_FRAGMENTS: usize = 4;

/// Finds the viewport point a press on the node would land on: the centre
/// of the visible part of each box `DOM.getContentQuads` reports, one per
/// line for a wrapped inline element, tried in order until one hit-tests to
/// the node. A node every tried point is covered at is the first refusal.
pub(crate) fn press_point(
    connection: &mut CdpConnection,
    session: &PageSession,
    token: &ElementToken,
    deadline: Deadline,
) -> Result<Point, AdapterError> {
    let viewport = ViewportMetrics::read(connection, session, deadline)?;
    let reply = connection
        .call(
            "DOM.getContentQuads",
            json!({ "backendNodeId": token.backend_node_id }),
            Some(&session.session_id),
            deadline,
        )
        .map_err(|error| not_rendered(error, token))?;
    let quads: Vec<Value> = reply
        .get("quads")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut points: Vec<Point> = quads
        .iter()
        .filter_map(|quad| viewport.visible_center(quad))
        .take(MAX_FRAGMENTS)
        .collect();
    if points.is_empty() {
        points.extend(quads.iter().find_map(|quad| quad_center(Some(quad))));
    }
    let mut refusal = None;
    for point in points {
        match confirm_hit(
            connection,
            session,
            token,
            &viewport.hit_point(point.clone()),
            deadline,
        ) {
            Ok(()) => return Ok(point),
            Err(error) if is_intercepted(&error) => {
                refusal.get_or_insert(error);
            }
            Err(error) => return Err(error),
        }
    }
    Err(refusal
        .unwrap_or_else(|| not_rendered_error(token.backend_node_id, "the node has an empty box")))
}

#[cfg(test)]
#[path = "press_point_tests.rs"]
mod tests;
