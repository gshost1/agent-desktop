use super::*;
use std::sync::mpsc;
use std::time::Duration;

static EXPIRED_IN_FLIGHT: AtomicBool = AtomicBool::new(false);
static BLOCKED_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

#[test]
fn space_functions_resolve_on_this_macos() {
    assert!(space_functions().is_some());
}

#[test]
fn a_window_number_with_no_window_is_on_no_space() {
    let deadline = Instant::now() + Duration::from_secs(5);
    assert_eq!(is_on_any_space(i64::from(u32::MAX), deadline), Some(false));
}

#[test]
fn a_window_number_outside_the_window_server_range_is_unknown() {
    let deadline = Instant::now() + Duration::from_secs(5);
    assert_eq!(is_on_any_space(i64::from(u32::MAX) + 1, deadline), None);
    assert_eq!(is_on_any_space(-1, deadline), None);
}

#[test]
fn an_expired_deadline_skips_the_space_read() {
    assert_eq!(
        read_before_deadline(&EXPIRED_IN_FLIGHT, Instant::now(), || panic!(
            "no read after the deadline"
        )),
        None
    );
}

#[test]
fn a_blocked_space_read_returns_unknown_at_the_deadline_and_blocks_no_second_read() {
    let (release, released) = mpsc::channel::<()>();
    let started = Instant::now();
    let blocked = read_before_deadline(
        &BLOCKED_IN_FLIGHT,
        started + Duration::from_millis(100),
        move || {
            let _ = released.recv_timeout(Duration::from_secs(10));
            Some(true)
        },
    );
    assert_eq!(blocked, None);
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(
        read_before_deadline(
            &BLOCKED_IN_FLIGHT,
            Instant::now() + Duration::from_secs(5),
            || { Some(false) }
        ),
        None
    );
    release.send(()).unwrap();
    let freed = Instant::now() + Duration::from_secs(5);
    while BLOCKED_IN_FLIGHT.load(Ordering::Acquire) && Instant::now() < freed {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        read_before_deadline(
            &BLOCKED_IN_FLIGHT,
            Instant::now() + Duration::from_secs(5),
            || { Some(false) }
        ),
        Some(false)
    );
}
