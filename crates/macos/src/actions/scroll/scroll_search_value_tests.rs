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

#[test]
fn an_item_that_moves_down_the_screen_during_a_search_down_went_backward() {
    use super::moved_backward;
    assert!(moved_backward(540.0, 730.0, &Direction::Down));
    assert!(!moved_backward(540.0, 340.0, &Direction::Down));
    assert!(!moved_backward(540.0, 540.2, &Direction::Down));
    assert!(moved_backward(340.0, 120.0, &Direction::Up));
    assert!(!moved_backward(340.0, 560.0, &Direction::Up));
    assert!(moved_backward(80.0, 200.0, &Direction::Right));
}

fn uncertain_write() -> agent_desktop_core::AdapterError {
    agent_desktop_core::AdapterError::new(
        agent_desktop_core::ErrorCode::AppUnresponsive,
        "AXValue returned kAXErrorCannotComplete; mutation outcome is uncertain",
    )
    .with_disposition(agent_desktop_core::DeliverySemantics::uncertain())
}

#[test]
fn a_refused_page_write_puts_the_probe_back_before_it_is_settled() {
    let mut writes = Vec::new();
    let step = calibrate(
        (0.4, 160.0),
        &Direction::Down,
        |value| {
            writes.push(value);
            match writes.len() {
                2 => Err(uncertain_write()),
                _ => Ok(true),
            }
        },
        || Some(22.0),
    );
    assert!(matches!(step, SearchStep::Refused(_)), "{step:?}");
    assert_eq!(writes.len(), 3, "{writes:?}");
    assert_eq!(writes[2], 0.4);
}

#[test]
fn a_probe_the_restore_did_not_put_back_is_not_a_sent_page() {
    let mut writes = Vec::new();
    let step = calibrate(
        (0.4, 160.0),
        &Direction::Down,
        |value| {
            writes.push(value);
            Ok(writes.len() == 1)
        },
        || Some(0.0),
    );
    assert!(
        !matches!(step, SearchStep::Sent | SearchStep::Skipped),
        "{step:?}"
    );
    assert_eq!(writes.len(), 2);
}

#[test]
fn a_restore_that_fails_keeps_the_probe_write_unsafe_to_retry() {
    let mut writes = 0;
    let step = calibrate(
        (0.4, 160.0),
        &Direction::Down,
        |_| {
            writes += 1;
            if writes == 1 {
                Ok(true)
            } else {
                Err(agent_desktop_core::AdapterError::new(
                    agent_desktop_core::ErrorCode::StaleRef,
                    "bar went away",
                )
                .with_disposition(agent_desktop_core::DeliverySemantics::not_delivered()))
            }
        },
        || Some(0.0),
    );
    let SearchStep::Displaced(error) = step else {
        panic!("a failed restore is displaced: {step:?}");
    };
    assert_eq!(error.code, agent_desktop_core::ErrorCode::StaleRef);
    assert_eq!(
        error.disposition,
        agent_desktop_core::DeliverySemantics::delivered_unverified()
    );
}
