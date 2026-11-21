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
            Self::DetectClamp => {
                super::window_ax_attributes::read_geometry_baseline(window, deadline)
                    .or_else(|| read_geometry(window, deadline).ok().flatten())
            }
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

    fn mismatch(self, observed: Rect) -> AdapterError {
        self.readback_error(
            Some(observed),
            "readback_mismatch",
            "The window manager left the window other than requested",
            "Read the held position and size from 'list-windows'",
        )
    }

    fn unavailable(self, observed: Option<Rect>) -> AdapterError {
        self.readback_error(
            observed,
            "readback_unavailable",
            "The window could not be read back after the request, so its effect is unknown",
            "Run 'list-windows' to see the window as it is now before sending the request again",
        )
    }

    fn readback_error(
        self,
        observed: Option<Rect>,
        cause: &str,
        message: &str,
        suggestion: &str,
    ) -> AdapterError {
        let requested = match self {
            Self::Move { x, y } => json!({ "x": x, "y": y }),
            Self::Resize { width, height } => json!({ "width": width, "height": height }),
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
            let bounds = crate::system::display::list_displays_impl(deadline)
                .unwrap_or_default()
                .into_iter()
                .map(|display| display.bounds)
                .collect::<Vec<_>>();
            add_display_bounds(details, &bounds);
        }
        error
    })
}

fn add_display_bounds(details: &mut serde_json::Value, bounds: &[Rect]) {
    if !bounds.is_empty() {
        details["display_bounds"] = json!(bounds);
    }
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
    let started = Instant::now();
    let mut last_readable: Option<(Rect, Instant)> = None;
    let mut stable = None;
    loop {
        let (actual, now) = read();
        let actual = match actual {
            Ok(actual) => actual,
            Err(error) if error.code == ErrorCode::AppUnresponsive => None,
            Err(error) if error.code == ErrorCode::Timeout => {
                return Err(last_readable.map_or_else(
                    || after_delivery(error),
                    |(frame, _)| request.unavailable(Some(frame)),
                ));
            }
            Err(error) => return Err(after_delivery(error)),
        };
        if let Some(actual) = actual {
            if request.matches(actual) {
                return Ok(());
            }
            last_readable = Some((actual, now));
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
                    return Err(request.mismatch(actual));
                }
                stable = Some((anchor, since, held));
            } else {
                stable = Some((actual, now, false));
            }
        } else {
            stable = None;
            let since = last_readable.map_or(started, |(_, last_read_at)| last_read_at);
            if now.duration_since(since) >= GEOMETRY_FALLBACK_TIME {
                return Err(request.unavailable(last_readable.map(|(frame, _)| frame)));
            }
        }
        let remaining = deadline.remaining();
        if remaining <= POLL {
            if let Some(frame) = actual {
                return Err(request.mismatch(frame));
            }
            if let Some((frame, _)) = last_readable {
                return Err(request.unavailable(Some(frame)));
            }
            if remaining.is_zero() {
                return Err(after_delivery(deadline.timeout_error()));
            }
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
