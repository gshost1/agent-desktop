use std::time::Duration;

use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode, Rect, WindowOp};
use serde_json::{Value, json};

use crate::system::x11::control::control_send::WindowWrite;
use crate::system::x11::control::window_facts::WindowFacts;

/// How far a read-back rectangle may sit from the requested one, in pixels,
/// and still count as the request satisfied.
pub(in crate::system) const POSITION_TOLERANCE: i32 = 2;

/// How often a read-back or a wait is repeated.
pub(in crate::system) const POLL: Duration = Duration::from_millis(25);

/// How long read-backs poll for a first match of a request. A read that does
/// not match at this budget gets one last look a poll later; a match seen at
/// any read then waits out its confirmation inside the command deadline.
pub(in crate::system) const SETTLE_BUDGET: Duration = Duration::from_millis(250);

const MAX_DIMENSION: f64 = 1_000_000.0;
const MAX_COORDINATE: f64 = 1_000_000.0;

fn near(wanted: i64, observed: i32) -> bool {
    (wanted - i64::from(observed)).abs() <= i64::from(POSITION_TOLERANCE)
}

fn field(wanted: Option<i64>, observed: i32) -> bool {
    wanted.is_none_or(|wanted| near(wanted, observed))
}

/// Whether a read-back of the window satisfies the write, within
/// [`POSITION_TOLERANCE`] for a rectangle.
pub(super) fn holds(write: &WindowWrite, observed: &WindowFacts) -> bool {
    match write {
        WindowWrite::MoveResize {
            x,
            y,
            width,
            height,
        } => {
            field(x.map(i64::from), observed.client.x)
                && field(y.map(i64::from), observed.client.y)
                && field(width.map(i64::from), observed.client.width)
                && field(height.map(i64::from), observed.client.height)
        }
        WindowWrite::Iconify => observed.hidden,
        WindowWrite::SetMaximized(true) => {
            observed.maximized_vertical && observed.maximized_horizontal
        }
        WindowWrite::SetMaximized(false) => {
            !observed.maximized_vertical && !observed.maximized_horizontal
        }
    }
}

fn rectangle(rect: &Rect) -> Value {
    json!({ "x": rect.x, "y": rect.y, "width": rect.width, "height": rect.height })
}

fn requested_of(write: &WindowWrite) -> Value {
    match write {
        WindowWrite::MoveResize {
            x,
            y,
            width,
            height,
        } => json!({ "x": x, "y": y, "width": width, "height": height }),
        WindowWrite::Iconify => json!({ "hidden": true }),
        WindowWrite::SetMaximized(on) => json!({ "maximized": on }),
    }
}

pub(in crate::system) fn observed_of(observed: Option<&WindowFacts>) -> Value {
    observed.map_or(Value::Null, |facts| {
        json!({
            "x": facts.client.x,
            "y": facts.client.y,
            "width": facts.client.width,
            "height": facts.client.height,
            "hidden": facts.hidden,
            "maximized_vertical": facts.maximized_vertical,
            "maximized_horizontal": facts.maximized_horizontal,
        })
    })
}

/// The error of a request whose read-back never held: what was asked, what
/// was seen, and for a rectangle the display bounds a clamp came from.
pub(in crate::system) fn unverified(
    write: &WindowWrite,
    observed: Option<&WindowFacts>,
    displays: &[Rect],
) -> AdapterError {
    let (cause, message, suggestion) = match observed {
        Some(_) => (
            "readback_mismatch",
            "The window manager left the window other than requested",
            "The window manager or the window's size hints constrained the request, and a window cannot be larger than the screen; read the held size from 'list-windows'",
        ),
        None => (
            "readback_unavailable",
            "The window could not be read back after the request, so its effect is unknown",
            "Run 'list-windows' to see the window as it is now before sending the request again",
        ),
    };
    let mut details = json!({
        "cause": cause,
        "requested": requested_of(write),
        "observed": observed_of(observed),
    });
    if observed.is_some() && matches!(write, WindowWrite::MoveResize { .. }) {
        details["display_bounds"] = displays.iter().map(rectangle).collect();
    }
    AdapterError::new(ErrorCode::ActionFailed, message)
        .with_suggestion(suggestion)
        .with_details(details)
        .with_disposition(DeliverySemantics::delivered_unverified())
}

/// The error of a request whose confirming read failed after a read-back
/// matched: its effect is unknown, and the read that matched is kept as what
/// was seen.
pub(in crate::system) fn unreadable_after(
    write: &WindowWrite,
    matched: &WindowFacts,
) -> AdapterError {
    let mut error = unverified(write, None, &[]);
    if let Some(Value::Object(details)) = error.details.as_mut() {
        details.insert("observed".to_owned(), observed_of(Some(matched)));
    }
    error
}

/// The error of a request whose read-back matched but could not be confirmed
/// before the deadline: the deadline's `timeout`, with what was asked and the
/// latest read-back, so a caller sees the window reached the place.
pub(in crate::system) fn unconfirmed(
    write: &WindowWrite,
    observed: &WindowFacts,
    timeout: AdapterError,
) -> AdapterError {
    let mut error = timeout.with_disposition(DeliverySemantics::delivered_unverified());
    if let Some(Value::Object(details)) = error.details.as_mut() {
        details.insert("requested".to_owned(), requested_of(write));
        details.insert("observed".to_owned(), observed_of(Some(observed)));
    }
    error
}

/// The error of a request whose read-back reached the place but kept moving
/// within the tolerance after the settle budget, while the deadline still had
/// time: the place was reached and not held still, so its effect is unverified.
pub(in crate::system) fn unsettled(write: &WindowWrite, observed: &WindowFacts) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "The window reached the requested place but kept moving, so it was not confirmed as held",
    )
    .with_suggestion(
        "The window manager is still adjusting the window; read its place from 'list-windows' before sending the request again",
    )
    .with_details(json!({
        "cause": "readback_unsettled",
        "requested": requested_of(write),
        "observed": observed_of(Some(observed)),
    }))
    .with_disposition(DeliverySemantics::delivered_unverified())
}

fn invalid(message: &str) -> AdapterError {
    AdapterError::new(ErrorCode::InvalidArgs, message)
        .with_disposition(DeliverySemantics::not_delivered())
}

/// The write an operation asks for, or `None` for a restore, which reads the
/// window first. A size or place that is not one is refused.
pub(in crate::system) fn requested(op: &WindowOp) -> Result<Option<WindowWrite>, AdapterError> {
    Ok(match *op {
        WindowOp::Resize { width, height } => {
            let valid = |side: f64| (0.5..=MAX_DIMENSION).contains(&side);
            if !valid(width) || !valid(height) {
                return Err(invalid(
                    "Window width and height must be finite, at least one pixel, and at most 1000000",
                ));
            }
            Some(WindowWrite::MoveResize {
                x: None,
                y: None,
                width: Some(width.round() as u32),
                height: Some(height.round() as u32),
            })
        }
        WindowOp::Move { x, y } => {
            let valid = |axis: f64| axis.is_finite() && axis.abs() <= MAX_COORDINATE;
            if !valid(x) || !valid(y) {
                return Err(invalid(
                    "Window coordinates must be finite and within -1000000..=1000000",
                ));
            }
            Some(WindowWrite::MoveResize {
                x: Some(x.round() as i32),
                y: Some(y.round() as i32),
                width: None,
                height: None,
            })
        }
        WindowOp::Minimize => Some(WindowWrite::Iconify),
        WindowOp::Maximize => Some(WindowWrite::SetMaximized(true)),
        WindowOp::Restore => None,
    })
}

#[cfg(test)]
#[path = "window_op_verdict_tests.rs"]
mod tests;
