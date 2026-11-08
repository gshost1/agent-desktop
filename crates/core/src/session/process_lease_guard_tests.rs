use std::time::{Duration, Instant};

use super::*;
use crate::ErrorCode;

fn refusal(result: Result<ProcessLeaseGuard, AdapterError>) -> AdapterError {
    match result {
        Ok(_) => panic!("the process lease was acquired while it was held"),
        Err(error) => error,
    }
}

fn kind(error: &AdapterError) -> String {
    error.details.as_ref().unwrap()["kind"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn acquire_from_another_thread(timeout_ms: u64) -> Result<ProcessLeaseGuard, AdapterError> {
    std::thread::spawn(move || ProcessLeaseGuard::acquire(Deadline::after(timeout_ms).unwrap()))
        .join()
        .unwrap()
}

#[test]
fn a_second_acquire_on_the_holding_thread_is_refused_at_once() {
    let _serial = test_serialization();
    let held = ProcessLeaseGuard::acquire(Deadline::after(5_000).unwrap()).unwrap();
    let started = Instant::now();
    let error = refusal(ProcessLeaseGuard::acquire(Deadline::after(5_000).unwrap()));
    let elapsed = started.elapsed();
    assert_eq!(error.code, ErrorCode::PolicyDenied, "after {elapsed:?}");
    assert_eq!(kind(&error), "reentrant_interaction_lease");
    assert!(elapsed < Duration::from_millis(100), "took {elapsed:?}");
    assert!(error.suggestion.is_some());
    assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
    let payload = crate::ErrorPayload::from_app_error(&crate::AppError::Adapter(error.clone()));
    assert!(payload.recovery.is_none());
    let still_held = refusal(acquire_from_another_thread(20));
    assert_eq!(kind(&still_held), "interaction_process_lock_timeout");
    drop(held);
    ProcessLeaseGuard::acquire(Deadline::after(1_000).unwrap()).unwrap();
}

#[test]
fn another_thread_still_waits_for_the_holder_and_acquires_after_release() {
    let _serial = test_serialization();
    let held = ProcessLeaseGuard::acquire(Deadline::after(5_000).unwrap()).unwrap();
    let error = refusal(acquire_from_another_thread(50));
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(kind(&error), "interaction_process_lock_timeout");
    assert!(
        error.details.as_ref().unwrap()["contention_count"]
            .as_u64()
            .unwrap()
            >= 1
    );
    let waiter = std::thread::spawn(|| {
        ProcessLeaseGuard::acquire(Deadline::after(5_000).unwrap())
            .map(|guard| guard.contention_count())
    });
    std::thread::sleep(Duration::from_millis(20));
    drop(held);
    assert!(waiter.join().unwrap().is_ok());
}

#[test]
fn a_guard_dropped_on_another_thread_frees_the_lease_for_its_first_owner() {
    let _serial = test_serialization();
    let held = ProcessLeaseGuard::acquire(Deadline::after(5_000).unwrap()).unwrap();
    std::thread::spawn(move || drop(held)).join().unwrap();
    ProcessLeaseGuard::acquire(Deadline::after(100).unwrap()).unwrap();
}

#[test]
fn a_guard_released_by_unwinding_frees_the_lease() {
    let _serial = test_serialization();
    let unwound = std::panic::catch_unwind(|| {
        let _held = ProcessLeaseGuard::acquire(Deadline::after(5_000).unwrap()).unwrap();
        panic!("unwind while holding the process lease");
    });
    assert!(unwound.is_err());
    ProcessLeaseGuard::acquire(Deadline::after(100).unwrap()).unwrap();
}
