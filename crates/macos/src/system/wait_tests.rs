use std::time::Instant;

use agent_desktop_core::{Deadline, MENU_OPENER_GRACE, MenuWait, ProcessIdentity};

use super::{forced_reads, wait_for_menu};

fn own_process() -> ProcessIdentity {
    let pid = std::process::id();
    let instance = crate::system::process::process_identity::token_for_pid(pid as i32)
        .unwrap()
        .unwrap();
    ProcessIdentity::new(pid, instance)
}

#[test]
fn a_closed_wait_holds_until_a_menu_that_opens_after_the_first_read_closes() {
    forced_reads::script(&[false, false, true, false, false]);
    let wait = MenuWait::closed(Instant::now());
    wait_for_menu(own_process(), wait, Deadline::after(5_000).unwrap()).unwrap();
    assert_eq!(forced_reads::taken(), 5);
}

#[test]
fn a_closed_wait_that_never_sees_a_menu_returns_once_the_grace_has_passed() {
    forced_reads::script(&[false]);
    let started = Instant::now();
    let wait = MenuWait::closed(started);
    wait_for_menu(own_process(), wait, Deadline::after(5_000).unwrap()).unwrap();
    assert!(started.elapsed() >= MENU_OPENER_GRACE);
    assert!(forced_reads::taken() > 1);
}

#[test]
fn a_deadline_inside_the_grace_after_closed_reads_is_closed_not_a_timeout() {
    forced_reads::script(&[false]);
    let wait = MenuWait::closed(Instant::now());
    wait_for_menu(own_process(), wait, Deadline::after(300).unwrap()).unwrap();
}
