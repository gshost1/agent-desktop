use std::cell::Cell;
use std::time::{Duration, Instant};

use agent_desktop_core::Deadline;

use super::*;
use crate::system::window::window_test_support::{pass, state as initial_state};

fn state(pid: u32) -> X11State {
    let mut state = initial_state(pid);
    if let X11State::Ready { clients, .. } = &mut state {
        let mut main = clients[0].clone();
        main.id = 1;
        main.title = "accessible name".into();
        clients.push(main);
    }
    state
}

#[test]
fn reread_stops_as_soon_as_focus_appears() {
    let calls = Cell::new(0);
    let (result, _) = reread_with(
        (pass(), state(7)),
        Deadline::standard().unwrap(),
        &[],
        |_| {
            calls.set(calls.get() + 1);
            let mut next = pass();
            next.focused = Some(0);
            Ok((next, state(7)))
        },
        |_| {},
    )
    .unwrap();
    assert_eq!(result.focused, Some(0));
    assert_eq!(calls.get(), 1);
}

#[test]
fn reread_stops_when_sources_no_longer_disagree() {
    let calls = Cell::new(0);
    reread_with(
        (pass(), state(7)),
        Deadline::standard().unwrap(),
        &[],
        |_| {
            calls.set(calls.get() + 1);
            Ok((pass(), state(8)))
        },
        |_| {},
    )
    .unwrap();
    assert_eq!(calls.get(), 1);
}

#[test]
fn reread_caps_attempts_and_bounds_the_supplied_deadline() {
    let calls = Cell::new(0);
    let pauses = Cell::new(Duration::ZERO);
    reread_with(
        (pass(), state(7)),
        Deadline::standard().unwrap(),
        &[],
        |until| {
            assert!(until.remaining() <= Duration::from_millis(300));
            calls.set(calls.get() + 1);
            let mut next = pass();
            next.windows[0].info.id = format!("frame-{}", calls.get());
            Ok((next, state(7)))
        },
        |duration| pauses.set(pauses.get() + duration),
    )
    .unwrap();
    assert_eq!(calls.get(), 3);
    assert_eq!(pauses.get(), Duration::from_millis(75));
}

#[test]
fn reread_does_not_wait_for_foreign_missing_or_already_focused_clients() {
    let mut focused = pass();
    focused.focused = Some(0);
    let mut empty = pass();
    empty.windows.clear();
    for initial in [
        (pass(), state(8)),
        (pass(), X11State::Unavailable),
        (
            pass(),
            X11State::Ready {
                active: None,
                clients: Vec::new(),
            },
        ),
        (focused, state(7)),
        (empty, state(7)),
    ] {
        reread_with(
            initial,
            Deadline::standard().unwrap(),
            &[],
            |_| panic!("must not read"),
            |_| panic!("must not wait"),
        )
        .unwrap();
    }
}

#[test]
fn reread_honors_short_or_expired_command_deadlines() {
    let expired = Deadline::after(1).unwrap();
    std::thread::sleep(Duration::from_millis(2));
    for deadline in [expired, Deadline::after(10).unwrap()] {
        reread_with(
            (pass(), state(7)),
            deadline,
            &[],
            |_| panic!("must not read"),
            |_| panic!("must not wait"),
        )
        .unwrap();
    }
}

#[test]
fn reread_checks_budget_again_after_waiting() {
    let started = Instant::now();
    reread_with(
        (pass(), state(7)),
        Deadline::after(35).unwrap(),
        &[],
        |_| panic!("must not read after expiry"),
        |_| std::thread::sleep(Duration::from_millis(40)),
    )
    .unwrap();
    assert!(started.elapsed() < Duration::from_millis(300));
}

#[test]
fn reread_preserves_the_last_valid_listing_when_a_read_times_out() {
    let calls = Cell::new(0);
    let (result, native) = reread_with(
        (pass(), state(7)),
        Deadline::standard().unwrap(),
        &[],
        |_| {
            calls.set(calls.get() + 1);
            if calls.get() == 1 {
                let mut next = pass();
                next.windows[0].info.id = "latest valid frame".into();
                next.windows[0].info.title = "latest valid listing".into();
                Ok((next, state(7)))
            } else {
                Err(AdapterError::timeout("read failed"))
            }
        },
        |_| {},
    )
    .unwrap();
    assert_eq!(result.windows[0].info.title, "latest valid listing");
    assert_eq!(active_client(&native).unwrap().1, 7);
    assert_eq!(calls.get(), 2);
}

#[test]
fn reread_preserves_an_initial_empty_listing_when_the_first_read_times_out() {
    let mut initial = pass();
    initial.windows.clear();
    let (result, _) = reread_with(
        (initial, state(7)),
        Deadline::standard().unwrap(),
        &[7],
        |_| Err(AdapterError::timeout("read failed")),
        |_| {},
    )
    .unwrap();
    assert!(result.windows.is_empty());
}

#[test]
fn reread_propagates_non_timeout_errors_instead_of_returning_an_earlier_listing() {
    for expected in [
        AdapterError::app_unresponsive("app"),
        AdapterError::not_supported("read"),
    ] {
        let error = reread_with(
            (pass(), state(7)),
            Deadline::standard().unwrap(),
            &[],
            |_| Err(expected.clone()),
            |_| {},
        )
        .err()
        .unwrap();
        assert_eq!(error.code, expected.code);
        assert_eq!(error.message, expected.message);
        assert_eq!(error.disposition, expected.disposition);
    }
}

#[test]
fn reread_can_recover_a_selected_process_with_no_listed_frames_yet() {
    let mut empty = pass();
    empty.windows.clear();
    let (result, _) = reread_with(
        (empty, state(7)),
        Deadline::standard().unwrap(),
        &[7],
        |_| {
            let mut next = pass();
            next.focused = Some(0);
            Ok((next, state(7)))
        },
        |_| {},
    )
    .unwrap();
    assert_eq!(result.focused, Some(0));
}

#[test]
fn persistent_unclaimed_frame_and_native_client_never_reread() {
    let mut persistent = pass();
    persistent.windows[0].frame.title = "different title".into();
    reread_with(
        (persistent, state(7)),
        Deadline::standard().unwrap(),
        &[7],
        |_| panic!("persistent identity mismatch must not read"),
        |_| panic!("persistent identity mismatch must not wait"),
    )
    .unwrap();
}

#[test]
fn new_unclaimed_client_rereads_claimed_frames_until_focus_appears() {
    let calls = Cell::new(0);
    let (result, _) = reread_with(
        (pass(), state(7)),
        Deadline::standard().unwrap(),
        &[7],
        |_| {
            calls.set(calls.get() + 1);
            let mut next = pass();
            next.windows[0].info.id = format!("mapping-{}", calls.get());
            if calls.get() == 2 {
                next.focused = Some(0);
            }
            Ok((next, state(7)))
        },
        |_| {},
    )
    .unwrap();
    assert_eq!(calls.get(), 2);
    assert_eq!(result.focused, Some(0));
}

#[test]
fn stable_frame_ids_and_active_client_stop_after_one_reread() {
    let calls = Cell::new(0);
    let mut initial = pass();
    let mut foreign = initial.windows[0].clone();
    foreign.info.id = "foreign".into();
    foreign.info.pid = 8.into();
    foreign.frame.pid = 8;
    initial.windows.push(foreign);
    let windows = initial.windows.clone();
    reread_with(
        (initial, state(7)),
        Deadline::standard().unwrap(),
        &[7],
        |_| {
            calls.set(calls.get() + 1);
            let mut next = pass();
            next.windows = windows.clone();
            next.windows.reverse();
            Ok((next, state(7)))
        },
        |_| {},
    )
    .unwrap();
    assert_eq!(calls.get(), 1);
}
