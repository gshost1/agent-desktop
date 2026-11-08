use agent_desktop_core::Rect;

use super::BoundsFit;

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn browser() -> Rect {
    rect(100.0, 50.0, 800.0, 600.0)
}

#[test]
fn device_pixels_at_100_percent_fit_exactly() {
    let platform = rect(101.0, 50.0, 800.0, 599.0);
    assert_eq!(BoundsFit::of(&browser(), &platform), Some(BoundsFit::Exact));
}

#[test]
fn device_pixels_at_150_percent_fit_at_that_scale() {
    let platform = rect(150.0, 75.0, 1200.0, 900.0);
    assert_eq!(
        BoundsFit::of(&browser(), &platform),
        Some(BoundsFit::Scaled)
    );
}

#[test]
fn device_pixels_at_200_percent_fit_at_that_scale() {
    let platform = rect(200.0, 100.0, 1600.0, 1200.0);
    assert_eq!(
        BoundsFit::of(&browser(), &platform),
        Some(BoundsFit::Scaled)
    );
}

#[test]
fn rounded_dips_far_from_the_origin_still_fit_at_150_percent() {
    let dips = rect(1333.0, 667.0, 801.0, 601.0);
    let platform = rect(2000.0, 1000.0, 1200.0, 900.0);
    assert_eq!(BoundsFit::of(&dips, &platform), Some(BoundsFit::Scaled));
}

#[test]
fn a_window_elsewhere_at_150_percent_fits_only_by_size() {
    let platform = rect(1500.0, 75.0, 1200.0, 900.0);
    assert_eq!(BoundsFit::of(&browser(), &platform), Some(BoundsFit::Sized));
}

#[test]
fn a_window_elsewhere_at_100_percent_fits_only_by_size() {
    let platform = rect(900.0, 50.0, 800.0, 600.0);
    assert_eq!(BoundsFit::of(&browser(), &platform), Some(BoundsFit::Sized));
}

#[test]
fn a_different_aspect_does_not_fit() {
    let platform = rect(150.0, 75.0, 1200.0, 1000.0);
    assert_eq!(BoundsFit::of(&browser(), &platform), None);
}

#[test]
fn a_platform_window_smaller_than_the_dips_does_not_fit() {
    let platform = rect(50.0, 25.0, 400.0, 300.0);
    assert_eq!(BoundsFit::of(&browser(), &platform), None);
}

#[test]
fn the_closer_fit_orders_first() {
    assert!(BoundsFit::Exact < BoundsFit::Scaled);
    assert!(BoundsFit::Scaled < BoundsFit::Sized);
}
