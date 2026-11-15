use super::{page_shift, shifted};
use agent_desktop_core::Direction;

#[test]
fn the_page_shift_moves_the_content_by_most_of_one_viewport() {
    let shift = page_shift(22.0, 160.0).unwrap();
    let moved_pixels = shift / 0.002 * 22.0;
    assert!((moved_pixels - 128.0).abs() < 1e-9, "{moved_pixels}");
}

#[test]
fn a_probe_that_moved_nothing_measurable_sizes_no_page() {
    assert_eq!(page_shift(0.0, 160.0), None);
    assert_eq!(page_shift(0.2, 160.0), None);
    assert_eq!(page_shift(10.0, 0.0), None);
}

#[test]
fn a_shift_stops_at_the_ends_of_the_bar() {
    assert_eq!(shifted(0.99, 0.05, &Direction::Down), 1.0);
    assert_eq!(shifted(0.01, 0.05, &Direction::Up), 0.0);
    assert!((shifted(0.5, 0.1, &Direction::Right) - 0.6).abs() < 1e-12);
}
