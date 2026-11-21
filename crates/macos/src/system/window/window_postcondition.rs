use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode, Rect};
use serde_json::json;
use std::time::{Duration, Instant};

pub(crate) const TOLERANCE: f64 = 2.0;
const GEOMETRY_SETTLE_TIME: Duration = Duration::from_millis(250);

#[derive(Clone, Copy)]
pub(super) enum GeometryRequest {
    Move { x: f64, y: f64 },
    Resize { width: f64, height: f64 },
}

impl GeometryRequest {
    fn matches(self, actual: Rect) -> bool {
        match self {
            Self::Move { x, y } => near(actual.x, x) && near(actual.y, y),
            Self::Resize { width, height } => {
                near(actual.width, width) && near(actual.height, height)
            }
        }
    }

    fn mismatch(self, observed: Rect) -> AdapterError {
        let requested = match self {
            Self::Move { x, y } => json!({ "x": x, "y": y }),
            Self::Resize { width, height } => json!({ "width": width, "height": height }),
        };
        AdapterError::new(
            ErrorCode::ActionFailed,
            "The window manager left the window other than requested",
        )
        .with_details(json!({
            "cause": "readback_mismatch",
            "requested": requested,
            "observed": observed,
        }))
    }
}

pub(super) fn read_geometry(
    element: &crate::tree::AXElement,
    deadline: Deadline,
) -> Result<Option<Rect>, AdapterError> {
    crate::tree::element_bounds::read_bounds_with_deadline(element, deadline_instant(deadline)?)
}

pub(super) fn wait_for_geometry(
    element: &crate::tree::AXElement,
    request: GeometryRequest,
    before: Option<Rect>,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    wait_for_geometry_with(request, before, deadline, || {
        Ok((read_geometry(element, deadline)?, Instant::now()))
    })
}

fn wait_for_geometry_with(
    request: GeometryRequest,
    before: Option<Rect>,
    deadline: Deadline,
    mut read: impl FnMut() -> Result<(Option<Rect>, Instant), AdapterError>,
) -> Result<(), AdapterError> {
    let mut stable = None;
    wait_until(deadline, || {
        let (actual, now) = read()?;
        let Some(actual) = actual else {
            stable = None;
            return Ok(false);
        };
        if request.matches(actual) {
            return Ok(true);
        }
        if before.is_none_or(|before| frame_matches(actual, before)) {
            stable = None;
            return Ok(false);
        }
        if let Some((anchor, since, confirm_on_next_read)) = stable
            && frame_matches(actual, anchor)
        {
            if confirm_on_next_read {
                return Err(request.mismatch(actual));
            }
            stable = Some((
                anchor,
                since,
                now.duration_since(since) >= GEOMETRY_SETTLE_TIME,
            ));
        } else {
            stable = Some((actual, now, false));
        }
        Ok(false)
    })
}

pub(crate) fn wait_for_minimized(
    element: &crate::tree::AXElement,
    expected: bool,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    wait_until(deadline, || {
        let actual = crate::tree::surface_read::boolean(
            element,
            "AXMinimized",
            deadline_instant(deadline)?,
        )?;
        Ok(actual == Some(expected))
    })
}

fn wait_until(
    deadline: Deadline,
    mut complete: impl FnMut() -> Result<bool, AdapterError>,
) -> Result<(), AdapterError> {
    loop {
        if complete().map_err(after_delivery)? {
            return Ok(());
        }
        if deadline.is_expired() {
            return Err(after_delivery(AdapterError::timeout(
                "Window operation did not reach its requested postcondition",
            )));
        }
        let pause = deadline
            .remaining_slice(Duration::from_millis(10))
            .map_err(after_delivery)?;
        std::thread::sleep(pause);
    }
}

fn frame_matches(actual: Rect, expected: Rect) -> bool {
    near(actual.x, expected.x)
        && near(actual.y, expected.y)
        && near(actual.width, expected.width)
        && near(actual.height, expected.height)
}

fn near(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= TOLERANCE
}

fn deadline_instant(deadline: Deadline) -> Result<Instant, AdapterError> {
    Instant::now()
        .checked_add(deadline.remaining())
        .ok_or_else(|| AdapterError::internal("Window operation deadline is out of range"))
}

fn after_delivery(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::delivered_unverified())
}

#[cfg(test)]
#[path = "window_postcondition_tests.rs"]
mod tests;
