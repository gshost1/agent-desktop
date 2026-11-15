use super::{SearchStep, calibrate, page_shift, shifted};
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

#[test]
fn a_probe_never_seen_to_move_puts_the_bar_back_and_sends_nothing() {
    let mut writes = Vec::new();
    let step = calibrate(
        (0.4, 160.0),
        &Direction::Down,
        |value| {
            writes.push(value);
            Ok(true)
        },
        || Some(0.0),
    );
    assert!(matches!(step, SearchStep::Skipped), "{step:?}");
    assert_eq!(writes.len(), 2);
    assert!((writes[0] - 0.402).abs() < 1e-12);
    assert_eq!(writes[1], 0.4);
}

#[test]
fn a_probe_that_moves_on_a_later_read_sizes_the_page() {
    let mut writes = Vec::new();
    let mut reads = vec![Some(0.0), None, Some(22.0)].into_iter();
    let step = calibrate(
        (0.4, 160.0),
        &Direction::Down,
        |value| {
            writes.push(value);
            Ok(true)
        },
        || reads.next().flatten(),
    );
    assert!(matches!(step, SearchStep::Sent), "{step:?}");
    assert_eq!(writes.len(), 2);
    let page = page_shift(22.0, 160.0).unwrap();
    assert!((writes[1] - (0.4 + page)).abs() < 1e-12);
}

#[test]
fn a_refused_probe_write_sends_nothing() {
    let step = calibrate(
        (0.4, 160.0),
        &Direction::Up,
        |_| Ok(false),
        || panic!("read after a refused write"),
    );
    assert!(matches!(step, SearchStep::Skipped), "{step:?}");
}
