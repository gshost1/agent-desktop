use std::time::Instant;

use agent_desktop_core::{Deadline, MENU_OPENER_GRACE, MenuWait, ProcessIdentity};

use super::{forced_reads, menu_open, wait_for_menu};

fn own_process() -> ProcessIdentity {
    let pid = std::process::id();
    let instance = crate::system::process::process_identity::token_for_pid(pid as i32)
        .unwrap()
        .unwrap();
    ProcessIdentity::new(pid, instance)
}

#[test]
fn a_closed_wait_holds_until_a_menu_that_opens_after_the_first_read_closes() {
    forced_reads::script(&[
        Some(false),
        Some(false),
        Some(true),
        Some(false),
        Some(false),
    ]);
    let wait = MenuWait::closed(Instant::now());
    wait_for_menu(own_process(), wait, Deadline::after(5_000).unwrap()).unwrap();
    assert_eq!(forced_reads::taken(), 5);
}

#[test]
fn a_closed_wait_that_never_sees_a_menu_returns_once_the_grace_has_passed() {
    forced_reads::script(&[Some(false)]);
    let started = Instant::now();
    let wait = MenuWait::closed(started);
    wait_for_menu(own_process(), wait, Deadline::after(5_000).unwrap()).unwrap();
    assert!(started.elapsed() >= MENU_OPENER_GRACE);
    assert!(forced_reads::taken() > 1);
}

#[test]
fn a_closed_wait_after_a_menu_item_click_returns_after_two_closed_reads() {
    forced_reads::script(&[Some(false)]);
    let started = Instant::now();
    let wait = MenuWait::closed_after_open();
    wait_for_menu(own_process(), wait, Deadline::after(5_000).unwrap()).unwrap();
    assert_eq!(forced_reads::taken(), 2);
    assert!(started.elapsed() < MENU_OPENER_GRACE);
}

#[test]
fn a_deadline_inside_the_grace_after_closed_reads_is_closed_not_a_timeout() {
    forced_reads::script(&[Some(false)]);
    let wait = MenuWait::closed(Instant::now());
    wait_for_menu(own_process(), wait, Deadline::after(1_000).unwrap()).unwrap();
}

#[test]
fn a_read_cut_by_the_deadline_after_closed_reads_is_closed_not_a_timeout() {
    forced_reads::script(&[Some(false), None]);
    let wait = MenuWait::closed(Instant::now());
    wait_for_menu(own_process(), wait, Deadline::after(1_000).unwrap()).unwrap();
}

#[test]
fn the_pre_action_reading_is_one_read_of_a_live_process() {
    forced_reads::script(&[Some(true)]);
    assert!(menu_open(&own_process(), Deadline::after(1_000).unwrap()).unwrap());
    assert_eq!(forced_reads::taken(), 1);
}
