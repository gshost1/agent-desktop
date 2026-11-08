//! Windows-only: the closed-direction opener grace in the real poll loop,
//! against a live `MenuFixture` whose menu stays closed. Each test holds
//! `FIXTURE_APP_NAME_LOCK` for its fixture's lifetime, as `wait_tests.rs` does.

use std::time::Instant;

use agent_desktop_core::{MENU_OPENER_GRACE, MenuWait, ProcessId};

use super::super::{poll_calls, wait_for_menu};
use super::{closed, deadline, identity_for};
use crate::system::fixture::test_support::FIXTURE_APP_NAME_LOCK;
use crate::tree::fixture::bootstrap;
use crate::tree::fixture::fixture_menu::MenuFixture;

/// Uses a poll-count assertion rather than an elapsed-time one so the
/// property holds regardless of host speed: once the opener grace is over, a
/// closed menu is read exactly once, proving the deadline is consulted only
/// after that first read rather than gating it.
#[test]
fn a_closed_wait_whose_grace_is_over_reads_an_already_closed_menu_once() {
    bootstrap();
    let _app_name_scope = FIXTURE_APP_NAME_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let fixture = MenuFixture::spawn().expect("the menu fixture starts");
    let identity = identity_for(ProcessId::from(fixture.process_id()));
    poll_calls::take();

    let result = wait_for_menu(identity, closed(), deadline(5_000));

    assert!(result.is_ok(), "the menu is closed and the grace is over");
    assert_eq!(poll_calls::take(), 1, "one read once the grace is over");
}

#[test]
fn a_closed_wait_inside_its_grace_keeps_polling_a_closed_menu_until_the_grace_ends() {
    bootstrap();
    let _app_name_scope = FIXTURE_APP_NAME_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let fixture = MenuFixture::spawn().expect("the menu fixture starts");
    let identity = identity_for(ProcessId::from(fixture.process_id()));
    poll_calls::take();
    let started = Instant::now();

    let result = wait_for_menu(identity, MenuWait::closed(started), deadline(10_000));

    assert!(
        result.is_ok(),
        "a menu that stays closed through the grace: {result:?}"
    );
    assert!(
        started.elapsed() >= MENU_OPENER_GRACE,
        "{:?}",
        started.elapsed()
    );
    assert!(
        poll_calls::take() > 1,
        "a closed read inside the grace must not end the wait"
    );
}

#[test]
fn a_deadline_inside_the_grace_after_closed_reads_is_closed_not_a_timeout() {
    bootstrap();
    let _app_name_scope = FIXTURE_APP_NAME_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let fixture = MenuFixture::spawn().expect("the menu fixture starts");
    let identity = identity_for(ProcessId::from(fixture.process_id()));

    let result = wait_for_menu(identity, MenuWait::closed(Instant::now()), deadline(1_000));

    assert!(
        result.is_ok(),
        "closed reads up to the deadline: {result:?}"
    );
}
