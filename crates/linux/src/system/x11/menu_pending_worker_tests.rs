use super::*;
use crate::tree::Extents;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

fn popup() -> PopupWindow {
    PopupWindow {
        id: 1,
        bounds: Extents {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        },
    }
}

fn poll(session: &MenuProbe, generation: usize) -> bool {
    session.requests.as_ref().unwrap().send(generation).unwrap();
    session
        .answers
        .recv_timeout(Duration::from_secs(1))
        .unwrap()
}

#[test]
fn a_completed_inventory_stays_pending_until_the_menu_settles() {
    let elapsed = Arc::new(AtomicUsize::new(0));
    let clock = elapsed.clone();
    let start = Instant::now();
    let session = MenuProbe::with_clock(
        || Some(|| Some(vec![popup()])),
        move || start + Duration::from_millis(clock.load(Ordering::SeqCst) as u64),
    );
    assert!(poll(&session, 1));
    elapsed.store(174, Ordering::SeqCst);
    assert!(poll(&session, 2));
    elapsed.store(175, Ordering::SeqCst);
    assert!(!poll(&session, 2));
}

#[test]
fn settling_without_a_subsequent_inventory_is_still_pending() {
    let elapsed = Arc::new(AtomicUsize::new(0));
    let clock = elapsed.clone();
    let start = Instant::now();
    let session = MenuProbe::with_clock(
        || Some(|| Some(vec![popup()])),
        move || start + Duration::from_millis(clock.load(Ordering::SeqCst) as u64),
    );
    assert!(poll(&session, 1));
    elapsed.store(200, Ordering::SeqCst);
    assert!(poll(&session, 1));
    assert!(!poll(&session, 2));
}

#[test]
fn reappearing_menu_restarts_its_settle_time() {
    let elapsed = Arc::new(AtomicUsize::new(0));
    let clock = elapsed.clone();
    let mapped = Arc::new(AtomicUsize::new(1));
    let state = mapped.clone();
    let start = Instant::now();
    let session = MenuProbe::with_clock(
        move || {
            let state = state.clone();
            Some(move || {
                Some(if state.load(Ordering::SeqCst) == 1 {
                    vec![popup()]
                } else {
                    vec![]
                })
            })
        },
        move || start + Duration::from_millis(clock.load(Ordering::SeqCst) as u64),
    );
    assert!(poll(&session, 1));
    elapsed.store(200, Ordering::SeqCst);
    assert!(!poll(&session, 2));
    mapped.store(0, Ordering::SeqCst);
    assert!(!poll(&session, 2));
    mapped.store(1, Ordering::SeqCst);
    assert!(poll(&session, 2));
    elapsed.store(374, Ordering::SeqCst);
    assert!(poll(&session, 3));
    elapsed.store(375, Ordering::SeqCst);
    assert!(!poll(&session, 3));
}

#[test]
fn a_failed_read_reconnects_once_and_recovers_evidence() {
    let connections = Arc::new(AtomicUsize::new(0));
    let count = connections.clone();
    let session = MenuProbe::with_reader(move || {
        let connection = count.fetch_add(1, Ordering::SeqCst);
        Some(move || (connection > 0).then(|| vec![popup()]))
    });
    assert!(poll(&session, 1));
    assert!(poll(&session, 1));
    assert_eq!(connections.load(Ordering::SeqCst), 2);
}

#[test]
fn a_second_failed_read_disables_the_worker() {
    let attempts = Arc::new(Mutex::new((0, 0)));
    let counts = attempts.clone();
    let session = MenuProbe::with_reader(move || {
        counts.lock().unwrap().0 += 1;
        let counts = counts.clone();
        Some(move || {
            counts.lock().unwrap().1 += 1;
            None
        })
    });
    session.requests.as_ref().unwrap().send(1).unwrap();
    assert!(
        session
            .answers
            .recv_timeout(Duration::from_secs(1))
            .is_err()
    );
    assert!(session.requests.as_ref().unwrap().send(2).is_err());
    assert_eq!(*attempts.lock().unwrap(), (2, 2));
}
