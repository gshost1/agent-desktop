use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, Rect};
use std::time::{Duration, Instant};

pub(crate) const TOLERANCE: f64 = 2.0;
const GEOMETRY_SETTLE_TIME: Duration = Duration::from_millis(100);

#[derive(Debug, PartialEq)]
pub(in crate::system) enum GeometryOutcome {
    Matched,
    Adjusted(Rect),
}

pub(super) fn wait_for_geometry(
    element: &crate::tree::AXElement,
    expected: Rect,
    position: bool,
    accept_adjustment: bool,
    deadline: Deadline,
) -> Result<GeometryOutcome, AdapterError> {
    wait_for_geometry_with(expected, position, accept_adjustment, deadline, || {
        crate::tree::element_bounds::read_bounds_with_deadline(element, deadline_instant(deadline)?)
    })
}

fn wait_for_geometry_with(
    expected: Rect,
    position: bool,
    accept_adjustment: bool,
    deadline: Deadline,
    mut read: impl FnMut() -> Result<Option<Rect>, AdapterError>,
) -> Result<GeometryOutcome, AdapterError> {
    let mut stable = None;
    wait_until(deadline, || {
        let Some(actual) = read()? else {
            stable = None;
            return Ok(None);
        };
        if geometry_matches(actual, expected, position) {
            return Ok(Some(GeometryOutcome::Matched));
        }
        if !accept_adjustment {
            return Ok(None);
        }
        let now = Instant::now();
        if let Some((previous, since)) = stable
            && geometry_matches(actual, previous, true)
            && geometry_matches(actual, previous, false)
        {
            return Ok((now.duration_since(since) >= GEOMETRY_SETTLE_TIME)
                .then_some(GeometryOutcome::Adjusted(actual)));
        }
        stable = Some((actual, now));
        Ok(None)
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
        Ok((actual == Some(expected)).then_some(()))
    })
}

fn wait_until<T>(
    deadline: Deadline,
    mut complete: impl FnMut() -> Result<Option<T>, AdapterError>,
) -> Result<T, AdapterError> {
    loop {
        if let Some(value) = complete().map_err(after_delivery)? {
            return Ok(value);
        }
        if deadline.is_expired() {
            return Err(after_delivery(AdapterError::timeout(
                "Window operation did not reach its requested postcondition",
            )));
        }
        let pause = deadline
            .remaining_slice(std::time::Duration::from_millis(10))
            .map_err(after_delivery)?;
        std::thread::sleep(pause);
    }
}

fn geometry_matches(actual: Rect, expected: Rect, position: bool) -> bool {
    let values = if position {
        [(actual.x, expected.x), (actual.y, expected.y)]
    } else {
        [
            (actual.width, expected.width),
            (actual.height, expected.height),
        ]
    };
    values
        .into_iter()
        .all(|(actual, expected)| (actual - expected).abs() <= TOLERANCE)
}

fn deadline_instant(deadline: Deadline) -> Result<std::time::Instant, AdapterError> {
    std::time::Instant::now()
        .checked_add(deadline.remaining())
        .ok_or_else(|| AdapterError::internal("Window operation deadline is out of range"))
}

fn after_delivery(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::delivered_unverified())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_verification_tolerates_ax_rounding_only() {
        let expected = Rect {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 80.0,
        };
        let rounded = Rect {
            x: 11.0,
            y: 19.0,
            width: 99.0,
            height: 81.0,
        };
        let wrong = Rect {
            width: 90.0,
            ..rounded
        };

        assert!(geometry_matches(rounded, expected, true));
        assert!(geometry_matches(rounded, expected, false));
        assert!(!geometry_matches(wrong, expected, false));
    }

    #[test]
    fn clamped_move_and_resize_return_the_observed_frame() {
        let observed = Rect {
            x: 350.0,
            y: 30.0,
            width: 723.0,
            height: 470.0,
        };
        for (position, expected) in [
            (
                true,
                Rect {
                    y: -200.0,
                    ..observed
                },
            ),
            (
                false,
                Rect {
                    width: 100.0,
                    height: 100.0,
                    ..observed
                },
            ),
        ] {
            let mut reads = 0;
            let outcome = wait_for_geometry_with(
                expected,
                position,
                true,
                Deadline::detached_after(1_000).unwrap(),
                || {
                    reads += 1;
                    Ok(Some(observed))
                },
            )
            .unwrap();
            assert!(reads > 1);
            assert_eq!(outcome, GeometryOutcome::Adjusted(observed));
        }
    }

    #[test]
    fn an_intermediate_frame_does_not_preempt_the_requested_frame() {
        let expected = Rect {
            x: 100.0,
            y: 40.0,
            width: 120.0,
            height: 90.0,
        };
        let intermediate = Rect {
            x: 80.0,
            ..expected
        };
        let mut samples = [
            intermediate,
            Rect {
                x: 90.0,
                ..intermediate
            },
            expected,
        ]
        .into_iter();
        let outcome = wait_for_geometry_with(
            expected,
            true,
            true,
            Deadline::detached_after(1_000).unwrap(),
            || Ok(samples.next()),
        )
        .unwrap();
        assert_eq!(outcome, GeometryOutcome::Matched);
    }

    #[test]
    fn a_missing_readback_still_times_out_after_dispatch() {
        let expected = Rect {
            x: 100.0,
            y: 40.0,
            width: 120.0,
            height: 90.0,
        };
        let error = wait_for_geometry_with(
            expected,
            true,
            true,
            Deadline::detached_after(20).unwrap(),
            || Ok(None),
        )
        .unwrap_err();
        assert_eq!(error.code, agent_desktop_core::ErrorCode::Timeout);
        assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    }
}
