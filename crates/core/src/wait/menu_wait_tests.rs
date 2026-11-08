use std::time::{Duration, Instant};

use super::{MENU_OPENER_GRACE, MenuWait};
use crate::MenuWaitPhase;

#[test]
fn an_open_wait_is_satisfied_by_the_first_open_read_only() {
    let now = Instant::now();
    let mut wait = MenuWait::opened();
    assert!(!wait.is_satisfied());
    assert!(!wait.observe(false, now));
    assert!(wait.observe(true, now));
}

#[test]
fn a_closed_wait_after_an_opener_needs_the_menu_to_open_then_close() {
    let now = Instant::now();
    let mut wait = MenuWait::closed(now, Duration::from_secs(30));
    let reads = [false, false, true, true, false];
    let satisfied: Vec<bool> = reads.iter().map(|open| wait.observe(*open, now)).collect();
    assert_eq!(satisfied, [false, false, false, false, true]);
}

#[test]
fn a_closed_wait_seen_open_before_the_action_ends_on_the_first_closed_read() {
    let mut wait = MenuWait::closed_after_open();
    assert!(wait.observe(false, Instant::now()));
}

#[test]
fn a_closed_wait_that_never_sees_a_menu_ends_once_the_grace_has_passed() {
    let started = Instant::now();
    let mut wait = MenuWait::closed(started, Duration::from_secs(30));
    assert!(!wait.observe(
        false,
        started + MENU_OPENER_GRACE - Duration::from_millis(1)
    ));
    assert!(wait.observe(false, started + MENU_OPENER_GRACE));
}

#[test]
fn the_grace_is_at_most_half_a_short_budget() {
    let started = Instant::now();
    let wait = MenuWait::closed(started, Duration::from_millis(400));
    assert_eq!(
        wait.phase(),
        MenuWaitPhase::AwaitingOpen {
            until: started + Duration::from_millis(200)
        }
    );
}

#[test]
fn only_the_closed_direction_wants_a_closed_menu() {
    assert!(MenuWait::opened().wants_open());
    assert!(!MenuWait::closed_after_open().wants_open());
}
