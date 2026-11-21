use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode, Rect};
use serde_json::json;
use std::time::{Duration, Instant};

pub(crate) const TOLERANCE: f64 = 2.0;
const GEOMETRY_SETTLE_TIME: Duration = Duration::from_millis(250);
const GEOMETRY_FALLBACK_TIME: Duration = Duration::from_secs(1);
const POLL: Duration = Duration::from_millis(10);

#[derive(Clone, Copy)]
pub(super) enum GeometryVerification {
    RequestedOnly,
    DetectClamp,
}

impl GeometryVerification {
    pub(super) fn baseline(
        self,
        window: &crate::tree::AXElement,
        deadline: Deadline,
    ) -> Option<Rect> {
        match self {
            Self::RequestedOnly => None,
            Self::DetectClamp => read_geometry(window, deadline).ok().flatten(),
        }
    }
}

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

    fn readback_error(self, observed: Option<Rect>) -> AdapterError {
        let requested = match self {
            Self::Move { x, y } => json!({ "x": x, "y": y }),
            Self::Resize { width, height } => json!({ "width": width, "height": height }),
        };
        let (cause, message, suggestion) = if observed.is_some() {
            (
                "readback_mismatch",
                "The window manager left the window other than requested",
                "Read the held position and size from 'list-windows'",
            )
        } else {
            (
                "readback_unavailable",
                "The window could not be read back after the request, so its effect is unknown",
                "Run 'list-windows' to see the window as it is now",
            )
        };
        AdapterError::new(ErrorCode::ActionFailed, message)
            .with_suggestion(suggestion)
            .with_details(json!({
                "cause": cause,
                "requested": requested,
                "observed": observed,
            }))
            .with_disposition(DeliverySemantics::delivered_unverified())
    }
}

fn read_geometry(
    element: &crate::tree::AXElement,
    deadline: Deadline,
) -> Result<Option<Rect>, AdapterError> {
    crate::tree::element_bounds::read_bounds_with_deadline(element, deadline_instant(deadline)?)
}

pub(super) fn wait_for_geometry(
    element: &crate::tree::AXElement,
    request: GeometryRequest,
    verification: GeometryVerification,
    before: Option<Rect>,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    if matches!(verification, GeometryVerification::RequestedOnly) {
        return wait_for_requested_geometry(request, deadline, || read_geometry(element, deadline));
    }
    wait_for_geometry_with(request, before, deadline, || {
        (read_geometry(element, deadline), Instant::now())
    })
    .map_err(|mut error| {
        if let Some(details) = &mut error.details
            && details["cause"] == "readback_mismatch"
        {
            let displays = crate::system::display::list_displays_impl(deadline).unwrap_or_default();
            details["display_bounds"] =
                json!(displays.into_iter().map(|d| d.bounds).collect::<Vec<_>>());
        }
        error
    })
}

fn wait_for_requested_geometry(
    request: GeometryRequest,
    deadline: Deadline,
    mut read: impl FnMut() -> Result<Option<Rect>, AdapterError>,
) -> Result<(), AdapterError> {
    wait_until(deadline, || {
        Ok(read()?.is_some_and(|frame| request.matches(frame)))
    })
}

fn wait_for_geometry_with(
    request: GeometryRequest,
    before: Option<Rect>,
    deadline: Deadline,
    mut read: impl FnMut() -> (Result<Option<Rect>, AdapterError>, Instant),
) -> Result<(), AdapterError> {
    let readback_limit = deadline.capped(GEOMETRY_FALLBACK_TIME);
    let mut last_readable = None;
    let mut stable = None;
    loop {
        let (actual, now) = read();
        let actual = match actual {
            Ok(actual) => actual,
            Err(error) if error.code == ErrorCode::AppUnresponsive => None,
            Err(error) if error.code == ErrorCode::Timeout && last_readable.is_some() => {
                return Err(request.readback_error(last_readable));
            }
            Err(error) => return Err(after_delivery(error)),
        };
        if let Some(actual) = actual {
            if request.matches(actual) {
                return Ok(());
            }
            last_readable = Some(actual);
            if let Some((anchor, since, confirm_on_next_read)) = stable
                && frame_matches(actual, anchor)
            {
                let quiet = if before.is_some_and(|before| {
                    !frame_matches(actual, before) && !frame_matches(anchor, before)
                }) {
                    GEOMETRY_SETTLE_TIME
                } else {
                    GEOMETRY_FALLBACK_TIME
                };
                let held = now.duration_since(since) >= quiet;
                if held && confirm_on_next_read {
                    return Err(request.readback_error(last_readable));
                }
                stable = Some((anchor, since, held));
            } else {
                stable = Some((actual, now, false));
            }
        } else {
            stable = None;
        }
        let remaining = deadline.remaining();
        if remaining <= POLL {
            if last_readable.is_some() {
                return Err(request.readback_error(last_readable));
            }
            if remaining.is_zero() {
                return Err(after_delivery(deadline.timeout_error()));
            }
        } else if last_readable.is_none() && readback_limit.is_expired() {
            return Err(request.readback_error(None));
        }
        std::thread::sleep(POLL.min(remaining));
    }
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
        let pause = deadline.remaining_slice(POLL).map_err(after_delivery)?;
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
