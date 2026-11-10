use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

static CHECK_STARTED: AtomicBool = AtomicBool::new(false);
static CALLER_DONE: AtomicBool = AtomicBool::new(false);

fn wait_for(flag: &AtomicBool) -> bool {
    let give_up = Instant::now() + Duration::from_secs(10);
    while !flag.load(Ordering::SeqCst) {
        if Instant::now() >= give_up {
            return false;
        }
        std::thread::yield_now();
    }
    true
}

#[test]
fn a_concurrent_check_returns_its_own_result() {
    let value = std::thread::scope(|scope| {
        let check = ConcurrentCheck::start(scope, || std::thread::current().id());
        (check.join(), std::thread::current().id())
    });
    assert_ne!(value.0, value.1);
}

#[test]
fn the_check_runs_while_the_caller_works() {
    let (started_before_join, check_saw_caller) = std::thread::scope(|scope| {
        let check = ConcurrentCheck::start(scope, || {
            CHECK_STARTED.store(true, Ordering::SeqCst);
            wait_for(&CALLER_DONE)
        });
        let started_before_join = wait_for(&CHECK_STARTED);
        CALLER_DONE.store(true, Ordering::SeqCst);
        (started_before_join, check.join())
    });
    assert!(started_before_join);
    assert!(check_saw_caller);
}
