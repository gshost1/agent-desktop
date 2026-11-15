use agent_desktop_core::{Direction, Rect};

use super::super::scroll_plan::{BlockedDirections, next_direction};
use super::{rect_has_area, scroll_effect_observed};

fn direction(target: Rect, viewport: Rect) -> Option<Direction> {
    next_direction(target, viewport, BlockedDirections::default())
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn area_requires_finite_positive_dimensions_and_coordinates() {
    assert!(rect_has_area(rect(0.0, 0.0, 10.0, 10.0)));
    assert!(!rect_has_area(rect(0.0, 0.0, 0.0, 10.0)));
    assert!(!rect_has_area(rect(0.0, 0.0, -1.0, 10.0)));
    assert!(!rect_has_area(rect(f64::NAN, 0.0, 10.0, 10.0)));
    assert!(!rect_has_area(rect(0.0, 0.0, f64::INFINITY, 10.0)));
}

#[test]
fn offscreen_direction_uses_global_viewport_edges() {
    let viewport = rect(1496.0, 87.0, 1496.0, 937.0);

    assert!(matches!(
        direction(rect(2030.0, 1026.0, 73.0, 24.0), viewport),
        Some(Direction::Down)
    ));
    assert!(matches!(
        direction(rect(2030.0, 80.0, 73.0, 24.0), viewport),
        Some(Direction::Up)
    ));
    assert!(direction(rect(1500.0, 100.0, 73.0, 24.0), viewport).is_none());
}

#[test]
fn nested_viewport_requires_scrolling_even_inside_window() {
    let target = rect(370.0, 466.0, 202.0, 32.0);
    let window = rect(300.0, 135.0, 500.0, 632.0);
    let viewport = rect(350.0, 617.0, 300.0, 100.0);

    assert!(direction(target, window).is_none());
    assert!(matches!(direction(target, viewport), Some(Direction::Up)));
}

#[test]
fn scroll_area_target_is_judged_against_its_enclosing_area() {
    let target_bounds = rect(360.0, 700.0, 200.0, 150.0);
    let enclosing_bounds = rect(350.0, 100.0, 500.0, 620.0);
    assert!(direction(target_bounds, target_bounds).is_none());
    assert!(matches!(
        direction(target_bounds, enclosing_bounds),
        Some(Direction::Down)
    ));
}

#[test]
fn only_a_definite_refusal_lets_scroll_into_view_try_another_direction() {
    let declined = agent_desktop_core::AdapterError::new(
        agent_desktop_core::ErrorCode::ActionNotSupported,
        "unavailable",
    )
    .with_disposition(agent_desktop_core::DeliverySemantics::not_delivered());
    assert!(super::imp::declined_without_delivery(&declined));

    let inert = agent_desktop_core::AdapterError::new(
        agent_desktop_core::ErrorCode::ActionNotSupported,
        "inert",
    );
    assert!(super::imp::declined_without_delivery(&inert));

    let uncertain = crate::actions::ax_mutation::classify_perform(
        "AXScrollLeftByPage",
        accessibility_sys::kAXErrorCannotComplete,
    )
    .unwrap_err();
    assert!(!super::imp::declined_without_delivery(&uncertain));

    let partial = agent_desktop_core::AdapterError::new(
        agent_desktop_core::ErrorCode::ActionNotSupported,
        "partial",
    )
    .with_disposition(agent_desktop_core::DeliverySemantics::delivered_unverified());
    assert!(!super::imp::declined_without_delivery(&partial));
}

#[test]
fn final_scroll_read_timeout_does_not_claim_safe_non_delivery() {
    let error = agent_desktop_core::AdapterError::timeout("final bounds read")
        .with_disposition(agent_desktop_core::DeliverySemantics::not_delivered());
    let error = super::imp::settled_scroll_outcome(None, Err(error)).unwrap_err();
    assert_eq!(error.code, agent_desktop_core::ErrorCode::Timeout);
    assert_eq!(
        error.disposition,
        agent_desktop_core::DeliverySemantics::delivered_unverified()
    );
}

#[test]
fn acknowledged_scroll_without_geometry_change_is_not_delivery() {
    let before = rect(2622.0, 1063.0, 177.0, 24.0);

    assert!(!scroll_effect_observed(Some(before), Some(before)));
    assert!(scroll_effect_observed(
        Some(before),
        Some(rect(2622.0, 900.0, 177.0, 24.0))
    ));
}

#[test]
fn scroll_area_whose_content_fits_horizontally_never_asks_for_a_horizontal_scroll() {
    let reported = rect(156.0, 88.0, 884.0, 720.0);
    let viewport = crate::tree::element_bounds::scroll_area_clip(reported, Some((884.0, 1273.5)));

    assert!(matches!(
        direction(rect(118.0, -227.0, 105.0, 20.0), viewport),
        Some(Direction::Up)
    ));
    assert!(direction(rect(118.0, 120.0, 105.0, 20.0), viewport).is_none());
}
