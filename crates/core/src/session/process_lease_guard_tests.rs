use std::sync::MutexGuard;
use std::time::Duration;

use super::*;
use crate::ErrorCode;
use crate::wait::deadline::VirtualClock;

fn process_lease_test_guard() -> MutexGuard<'static, ()> {
    PROCESS_LEASE_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[test]
fn a_free_lease_taken_after_the_deadline_reports_a_deadline_expiry() {
    let _guard = process_lease_test_guard();
    let clock = VirtualClock::freeze();
    let deadline = Deadline::after(10).unwrap();
    clock.advance(Duration::from_millis(15));

    let error = match ProcessLeaseGuard::acquire(deadline) {
        Ok(_) => panic!("an expired deadline must not hand out the lease"),
        Err(error) => error,
    };

    assert_eq!(error.code, ErrorCode::Timeout);
    let details = error.details.expect("details");
    assert_eq!(details["kind"], "deadline");
    assert_eq!(details["timeout_ms"], 10);
    assert_eq!(details["elapsed_ms"], 15);
    assert!(details.get("contention_count").is_none());
    assert!(
        ProcessLeaseGuard::acquire(Deadline::after(10).unwrap()).is_ok(),
        "the refused acquire must release the lease it briefly took"
    );
}

#[test]
fn a_wait_on_a_held_lease_reports_lock_contention() {
    let _guard = process_lease_test_guard();
    let clock = VirtualClock::freeze();
    let holder = ProcessLeaseGuard::acquire(Deadline::after(1_000).unwrap()).unwrap();
    let deadline = Deadline::after(10).unwrap();

    let error = match ProcessLeaseGuard::acquire(deadline) {
        Ok(_) => panic!("a held lease must not be handed out twice"),
        Err(error) => error,
    };

    assert_eq!(error.code, ErrorCode::Timeout);
    let details = error.details.expect("details");
    assert_eq!(details["kind"], "interaction_process_lock_timeout");
    assert!(details["contention_count"].as_u64().unwrap() >= 1);
    assert!(clock.elapsed() >= Duration::from_millis(10));
    drop(holder);
}

#[test]
fn a_held_lease_with_an_expired_deadline_still_reports_lock_contention() {
    let _guard = process_lease_test_guard();
    let clock = VirtualClock::freeze();
    let holder = ProcessLeaseGuard::acquire(Deadline::after(1_000).unwrap()).unwrap();
    let deadline = Deadline::after(10).unwrap();
    clock.advance(Duration::from_millis(15));

    let error = match ProcessLeaseGuard::acquire(deadline) {
        Ok(_) => panic!("a held lease must not be handed out twice"),
        Err(error) => error,
    };

    let details = error.details.expect("details");
    assert_eq!(details["kind"], "interaction_process_lock_timeout");
    assert_eq!(details["contention_count"], 1);
    drop(holder);
}

#[test]
fn a_lease_taken_after_waiting_out_the_deadline_reports_lock_contention() {
    let clock = VirtualClock::freeze();
    let deadline = Deadline::after(10).unwrap();
    clock.advance(Duration::from_millis(10));

    let details = expired_on_acquire(deadline, 3).details.expect("details");

    assert_eq!(details["kind"], "interaction_process_lock_timeout");
    assert_eq!(details["contention_count"], 3);
}
