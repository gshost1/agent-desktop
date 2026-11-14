use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode, Point};
use serde_json::{Value, json};

use super::element_errors::node_error;
use super::remote_element::prove_document;
use crate::cdp::protocol_error::{is_node_gone, is_protocol_error};
use crate::{CdpConnection, ElementToken, PageSession};

/// Scrolls the node into the viewport if it is not already visible.
pub(crate) fn scroll_into_view(
    connection: &mut CdpConnection,
    session: &PageSession,
    token: &ElementToken,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    prove_document(connection, session, token, deadline)?;
    scroll_node(connection, session, token, deadline)
}

pub(crate) fn scroll_node(
    connection: &mut CdpConnection,
    session: &PageSession,
    token: &ElementToken,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    connection
        .call(
            "DOM.scrollIntoViewIfNeeded",
            json!({ "backendNodeId": token.backend_node_id }),
            Some(&session.session_id),
            deadline,
        )
        .map(|_| ())
        .map_err(|error| not_rendered(error, token))
}

pub(crate) fn quad_center(quad: Option<&Value>) -> Option<Point> {
    let numbers: Vec<f64> = quad?.as_array()?.iter().filter_map(Value::as_f64).collect();
    if numbers.len() != 8 {
        return None;
    }
    let xs = [numbers[0], numbers[2], numbers[4], numbers[6]];
    let ys = [numbers[1], numbers[3], numbers[5], numbers[7]];
    let width =
        xs.iter().copied().fold(f64::MIN, f64::max) - xs.iter().copied().fold(f64::MAX, f64::min);
    let height =
        ys.iter().copied().fold(f64::MIN, f64::max) - ys.iter().copied().fold(f64::MAX, f64::min);
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    Some(Point {
        x: xs.iter().sum::<f64>() / 4.0,
        y: ys.iter().sum::<f64>() / 4.0,
    })
}

pub(crate) fn not_rendered(error: AdapterError, token: &ElementToken) -> AdapterError {
    if is_node_gone(&error) || !is_protocol_error(&error) {
        return node_error(error, token);
    }
    let detail = error.platform_detail.clone().unwrap_or_default();
    not_rendered_error(token.backend_node_id, &detail)
}

pub(crate) fn not_rendered_error(backend: i64, detail: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        format!("DOM node {backend} has no rendered box to act on"),
    )
    .with_suggestion("The element may be hidden or detached from layout; refresh the snapshot")
    .with_platform_detail(detail)
    .with_details(json!({ "kind": "cdp_node_not_rendered", "backend_node_id": backend }))
    .with_disposition(DeliverySemantics::not_delivered())
}

#[cfg(test)]
#[path = "scroll_tests.rs"]
mod tests;
