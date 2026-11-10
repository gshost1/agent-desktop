use agent_desktop_core::{Direction, Rect};

use super::{BlockedDirections, bar_can_move, next_direction};

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn blocked(directions: &[Direction]) -> BlockedDirections {
    let mut blocked = BlockedDirections::default();
    for direction in directions {
        blocked.block(direction);
    }
    blocked
}

#[test]
fn target_above_a_viewport_whose_frame_starts_right_of_it_scrolls_up_first() {
    let viewport = rect(156.0, 88.0, 884.0, 720.0);
    let target = rect(118.0, -227.0, 105.0, 20.0);

    assert!(matches!(
        next_direction(target, viewport, BlockedDirections::default()),
        Some(Direction::Up)
    ));
}

#[test]
fn horizontal_clip_that_the_ancestor_cannot_scroll_is_dropped() {
    let viewport = rect(156.0, 88.0, 884.0, 720.0);
    let revealed = rect(118.0, 120.0, 105.0, 20.0);

    assert!(matches!(
        next_direction(revealed, viewport, BlockedDirections::default()),
        Some(Direction::Left)
    ));
    assert!(next_direction(revealed, viewport, blocked(&[Direction::Left])).is_none());
}

#[test]
fn axis_that_hides_the_target_goes_before_an_axis_that_only_clips_it() {
    let viewport = rect(0.0, 0.0, 400.0, 300.0);

    assert!(matches!(
        next_direction(
            rect(500.0, 290.0, 50.0, 20.0),
            viewport,
            BlockedDirections::default()
        ),
        Some(Direction::Right)
    ));
    assert!(matches!(
        next_direction(
            rect(-80.0, 290.0, 50.0, 20.0),
            viewport,
            BlockedDirections::default()
        ),
        Some(Direction::Left)
    ));
    assert!(matches!(
        next_direction(
            rect(390.0, 400.0, 50.0, 20.0),
            viewport,
            BlockedDirections::default()
        ),
        Some(Direction::Down)
    ));
    assert!(matches!(
        next_direction(
            rect(500.0, 400.0, 50.0, 20.0),
            viewport,
            BlockedDirections::default()
        ),
        Some(Direction::Down)
    ));
}

#[test]
fn horizontally_offscreen_target_scrolls_on_the_horizontal_axis() {
    let viewport = rect(40.0, 88.0, 1000.0, 720.0);

    assert!(matches!(
        next_direction(
            rect(1200.0, 300.0, 80.0, 20.0),
            viewport,
            BlockedDirections::default()
        ),
        Some(Direction::Right)
    ));
    assert!(matches!(
        next_direction(
            rect(-300.0, 300.0, 80.0, 20.0),
            viewport,
            BlockedDirections::default()
        ),
        Some(Direction::Left)
    ));
}

#[test]
fn blocked_axis_falls_through_to_the_other_axis() {
    let viewport = rect(0.0, 0.0, 400.0, 300.0);
    let target = rect(500.0, 400.0, 50.0, 20.0);

    assert!(matches!(
        next_direction(target, viewport, blocked(&[Direction::Down])),
        Some(Direction::Right)
    ));
    assert!(
        next_direction(
            target,
            viewport,
            blocked(&[Direction::Down, Direction::Right])
        )
        .is_none()
    );
    assert!(matches!(
        next_direction(target, viewport, blocked(&[Direction::Up, Direction::Left])),
        Some(Direction::Down)
    ));
}

#[test]
fn target_larger_than_the_viewport_on_an_axis_needs_no_scroll_on_it() {
    let viewport = rect(0.0, 0.0, 400.0, 300.0);

    assert!(
        next_direction(
            rect(10.0, -50.0, 100.0, 500.0),
            viewport,
            BlockedDirections::default()
        )
        .is_none()
    );
    assert!(
        next_direction(
            rect(f64::NAN, 10.0, 100.0, 20.0),
            viewport,
            BlockedDirections::default()
        )
        .is_none()
    );
}

#[test]
fn scroll_bar_at_a_limit_cannot_move_past_it() {
    assert!(!bar_can_move(0.0, &Direction::Up));
    assert!(!bar_can_move(0.0, &Direction::Left));
    assert!(bar_can_move(0.0, &Direction::Down));
    assert!(!bar_can_move(1.0, &Direction::Down));
    assert!(!bar_can_move(1.0, &Direction::Right));
    assert!(bar_can_move(1.0, &Direction::Up));
    assert!(bar_can_move(0.5, &Direction::Left));
    assert!(bar_can_move(f64::NAN, &Direction::Up));
}
