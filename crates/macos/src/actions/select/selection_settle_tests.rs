use agent_desktop_core::Deadline;

use super::{SELECTION_SETTLE_MS, selection_settles};

#[test]
fn a_selection_that_lands_after_the_first_read_is_verified() {
    let mut reads = 0;
    let settled = selection_settles(Deadline::after(5_000).unwrap(), || {
        reads += 1;
        reads >= 3
    });
    assert!(settled);
    assert_eq!(reads, 3);
}

#[test]
fn an_immediate_selection_reads_once() {
    let mut reads = 0;
    assert!(selection_settles(Deadline::after(5_000).unwrap(), || {
        reads += 1;
        true
    }));
    assert_eq!(reads, 1);
}

#[test]
fn a_selection_that_never_lands_stops_at_the_settle_window() {
    let started = std::time::Instant::now();
    assert!(!selection_settles(Deadline::after(5_000).unwrap(), || {
        false
    }));
    let elapsed = started.elapsed().as_millis();
    assert!(elapsed >= u128::from(SELECTION_SETTLE_MS));
    assert!(elapsed < 2_000, "settle ran {elapsed} ms");
}

#[test]
fn an_expired_deadline_still_reads_once_and_never_waits() {
    let deadline = Deadline::after(1).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));
    let mut reads = 0;
    assert!(!selection_settles(deadline, || {
        reads += 1;
        false
    }));
    assert_eq!(reads, 1);
}

#[test]
fn a_direct_write_whose_readback_lands_late_is_verified() {
    let mut reads = 0;
    let holds = super::direct_selection_holds(Deadline::after(5_000).unwrap(), || {
        reads += 1;
        Some(reads >= 2)
    });
    assert!(holds);
    assert_eq!(reads, 2);
}

#[test]
fn a_direct_write_with_no_readable_selection_is_not_verified() {
    assert!(!super::direct_selection_holds(
        Deadline::after(5_000).unwrap(),
        || None
    ));
}
