use std::time::{Duration, Instant};

use super::{MENU_OPENER_GRACE, MenuWait};
use crate::wait::menu_wait_phase::MenuWaitPhase;

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
    let mut wait = MenuWait::closed(now);
    let reads = [false, false, true, true, false, false];
    let satisfied: Vec<bool> = reads.iter().map(|open| wait.observe(*open, now)).collect();
    assert_eq!(satisfied, [false, false, false, false, false, true]);
}

#[test]
fn a_menu_swapped_for_another_across_one_poll_is_not_a_close() {
    let now = Instant::now();
    let mut wait = MenuWait::closed_after_open();
    let reads = [false, true, false, false];
    let satisfied: Vec<bool> = reads.iter().map(|open| wait.observe(*open, now)).collect();
    assert_eq!(satisfied, [false, false, false, true]);
}

#[test]
fn a_closed_wait_that_never_sees_a_menu_ends_once_the_grace_has_passed() {
    let started = Instant::now();
    let mut wait = MenuWait::closed(started);
    assert!(!wait.observe(
        false,
        started + MENU_OPENER_GRACE - Duration::from_millis(1)
    ));
    assert!(wait.observe(false, started + MENU_OPENER_GRACE));
}

#[test]
fn the_grace_is_the_full_opener_grace_whatever_the_budget() {
    let started = Instant::now();
    assert_eq!(
        MenuWait::closed(started).phase(),
        MenuWaitPhase::AwaitingOpen {
            until: started + Duration::from_millis(1_500),
            read_closed: false,
        }
    );
}

#[test]
fn the_deadline_satisfies_a_closed_wait_only_after_a_closed_read() {
    let started = Instant::now();
    let mut unread = MenuWait::closed(started);
    assert!(!unread.satisfied_at_deadline());
    let mut read = MenuWait::closed(started);
    assert!(!read.observe(false, started));
    assert!(read.satisfied_at_deadline());
    let mut seen_open = MenuWait::closed(started);
    assert!(!seen_open.observe(true, started));
    assert!(!seen_open.satisfied_at_deadline());
    assert!(!MenuWait::opened().satisfied_at_deadline());
}

#[test]
fn only_the_open_direction_wants_an_open_menu() {
    assert!(MenuWait::opened().wants_open());
    assert!(!MenuWait::closed_after_open().wants_open());
}
