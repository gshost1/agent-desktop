use agent_desktop_core::{AdapterError, OperationReport, WindowInfo, WindowOp, WindowReadback};
use serde_json::json;

use super::window_postcondition::{GeometryOutcome, TOLERANCE};

pub(in crate::system) fn report(
    window: &WindowInfo,
    op: &WindowOp,
    dispatched: Result<Option<GeometryOutcome>, AdapterError>,
) -> Result<OperationReport, AdapterError> {
    let outcome = match dispatched {
        Ok(outcome) => outcome,
        Err(error) => return OperationReport::from_window_dispatch(window, op, None, Err(error)),
    };
    let Some(GeometryOutcome::Adjusted(actual)) = outcome else {
        return OperationReport::from_window_dispatch(window, op, window_readback(op), Ok(()));
    };
    let bounds = window.bounds;
    let requested = match op {
        WindowOp::Move { x, y } => json!({
            "x": x, "y": y,
            "width": bounds.map(|frame| frame.width),
            "height": bounds.map(|frame| frame.height),
        }),
        WindowOp::Resize { width, height } => json!({
            "x": bounds.map(|frame| frame.x),
            "y": bounds.map(|frame| frame.y),
            "width": width, "height": height,
        }),
        WindowOp::Minimize | WindowOp::Maximize | WindowOp::Restore => {
            return OperationReport::from_window_dispatch(window, op, window_readback(op), Ok(()));
        }
    };
    let mut report = OperationReport::from_window_dispatch(window, op, None, Ok(()))?;
    let mut details = report.result.details.take().unwrap_or_else(|| json!({}));
    details["geometry_outcome"] = json!("settled_different_frame");
    details["requested_frame"] = requested;
    details["actual_frame"] = json!(actual);
    details["units"] = json!("points");
    report.result.details = Some(details);
    Ok(report)
}

fn window_readback(op: &WindowOp) -> Option<WindowReadback> {
    match op {
        WindowOp::Resize { .. } => {
            Some(WindowReadback::geometry("window_size", TOLERANCE, "points"))
        }
        WindowOp::Move { .. } => Some(WindowReadback::geometry(
            "window_position",
            TOLERANCE,
            "points",
        )),
        WindowOp::Minimize | WindowOp::Restore => {
            Some(WindowReadback::state("window_minimized_state"))
        }
        WindowOp::Maximize => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_desktop_core::{
        DeliverySemantics, OperationTarget, ProcessIdentity, Rect, WindowState,
    };

    fn target() -> OperationTarget {
        OperationTarget::Window {
            id: "w-1".into(),
            title: "Doc".into(),
            process: ProcessIdentity::new(7u32, "boot"),
        }
    }

    fn report_with_matching_readback(op: &WindowOp) -> OperationReport {
        OperationReport::window_operation(target(), op, window_readback(op))
    }

    #[test]
    fn maximize_stays_unverified_because_position_and_size_are_read_separately() {
        assert_eq!(
            report_with_matching_readback(&WindowOp::Maximize).disposition(),
            DeliverySemantics::delivered_unverified()
        );
    }

    #[test]
    fn geometry_scopes_name_exactly_what_was_read_with_the_real_tolerance() {
        let resize = report_with_matching_readback(&WindowOp::Resize {
            width: 3.0,
            height: 4.0,
        });
        let moved = report_with_matching_readback(&WindowOp::Move { x: 1.0, y: 2.0 });

        let resize = resize.result.details.unwrap();
        let moved = moved.result.details.unwrap();
        assert_eq!(resize["verification_scope"], "window_size");
        assert_eq!(moved["verification_scope"], "window_position");
        for details in [resize, moved] {
            assert_eq!(details["tolerance"], TOLERANCE);
            assert_eq!(details["units"], "points");
        }
    }

    #[test]
    fn minimize_and_restore_report_the_minimized_state_without_a_tolerance() {
        for op in [WindowOp::Minimize, WindowOp::Restore] {
            let details = report_with_matching_readback(&op).result.details.unwrap();
            assert_eq!(details["verification_scope"], "window_minimized_state");
            assert!(details.get("tolerance").is_none());
        }
    }

    #[test]
    fn adjusted_geometry_reports_both_frames_without_claiming_the_request_was_met() {
        let before = Rect {
            x: 350.0,
            y: 557.0,
            width: 723.0,
            height: 625.0,
        };
        let actual = Rect {
            y: 30.0,
            height: 470.0,
            ..before
        };
        let window = WindowInfo {
            id: "w-1".into(),
            title: "Doc".into(),
            app: "App".into(),
            pid: 7u32.into(),
            process_instance: Some("boot".into()),
            bounds: Some(before),
            state: WindowState::default(),
        };

        for (op, requested) in [
            (
                WindowOp::Move {
                    x: 350.0,
                    y: -200.0,
                },
                Rect {
                    y: -200.0,
                    ..before
                },
            ),
            (
                WindowOp::Resize {
                    width: 100.0,
                    height: 100.0,
                },
                Rect {
                    width: 100.0,
                    height: 100.0,
                    ..before
                },
            ),
        ] {
            let report = report(&window, &op, Ok(Some(GeometryOutcome::Adjusted(actual)))).unwrap();
            assert_eq!(
                report.disposition(),
                DeliverySemantics::delivered_unverified()
            );
            let details = report.result.details.unwrap();
            assert_eq!(details["geometry_outcome"], "settled_different_frame");
            assert_eq!(details["requested_frame"], json!(requested));
            assert_eq!(details["actual_frame"], json!(actual));
            assert_eq!(details["units"], "points");
        }
    }
}
