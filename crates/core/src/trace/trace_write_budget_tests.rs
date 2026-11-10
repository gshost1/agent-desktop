use super::*;
use crate::refs::refs_test_support::HomeGuard;
use crate::wait::deadline::{VirtualClock, enter_scope};
use std::path::PathBuf;
use std::time::Duration;

fn session_trace_dir(session: &str) -> PathBuf {
    let dir = crate::refs::refs_store::RefStore::for_session(Some(session))
        .unwrap()
        .trace_dir();
    crate::trace::trace_file::ensure_trace_dir(&dir).unwrap();
    dir
}

fn hold_budget_lock(dir: &Path) -> RefStoreLock {
    RefStoreLock::acquire_with_deadline(
        &dir.join(".trace-budget.lock"),
        Deadline::detached_after(1_000).unwrap(),
    )
    .unwrap()
}

#[test]
fn a_contended_trace_lock_waits_for_the_command_deadline() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let dir = session_trace_dir("contended-command");
    let _held = hold_budget_lock(&dir);
    let _scope = enter_scope(Some(Deadline::after(1_000).unwrap()));
    let error = match TraceWriteBudget::acquire(&dir) {
        Ok(_) => panic!("a held budget lock must not be acquired"),
        Err(error) => error,
    };
    assert_eq!(error.code(), "TIMEOUT");
    assert_eq!(clock.elapsed(), Duration::from_millis(1_000));
}

#[test]
fn a_trace_lock_wait_never_outlasts_the_store_lock_bound() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let dir = session_trace_dir("contended-long-command");
    let _held = hold_budget_lock(&dir);
    let _scope = enter_scope(Some(Deadline::after(30_000).unwrap()));
    assert!(TraceWriteBudget::acquire(&dir).is_err());
    assert_eq!(clock.elapsed(), Duration::from_millis(LOCK_TIMEOUT_MS));
}

#[test]
fn a_spent_command_deadline_still_leaves_the_trace_lock_its_floor() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let dir = session_trace_dir("contended-spent");
    let _held = hold_budget_lock(&dir);
    let _scope = enter_scope(Some(Deadline::after(0).unwrap()));
    assert!(TraceWriteBudget::acquire(&dir).is_err());
    assert_eq!(clock.elapsed(), Duration::from_millis(TRACE_LOCK_FLOOR_MS));
}

#[test]
fn a_spent_command_deadline_does_not_drop_an_uncontended_record() {
    let _home = HomeGuard::new();
    let _clock = VirtualClock::freeze();
    let dir = session_trace_dir("uncontended-spent");
    let _scope = enter_scope(Some(Deadline::after(0).unwrap()));
    let mut budget = TraceWriteBudget::acquire(&dir).unwrap();
    budget.reserve_line(40).unwrap();
    assert_eq!(budget.usage.1, 1);
}

#[test]
fn the_ledger_keeps_one_fixed_width_record_across_rewrites() {
    let _home = HomeGuard::new();
    let dir = session_trace_dir("fixed-width");
    let ledger = dir.join(LEDGER);
    {
        let mut budget = TraceWriteBudget::acquire(&dir).unwrap();
        budget.reserve_line(7).unwrap();
        budget.reserve_line(123_456).unwrap();
    }
    let record = std::fs::read(&ledger).unwrap();
    assert_eq!(record.len(), encode_ledger((0, 0, 0)).len());
    let usage: (usize, usize, usize) = serde_json::from_slice(&record).unwrap();
    assert_eq!((usage.0, usage.1), (123_463, 2));
    assert_eq!(
        encode_ledger((usize::MAX, usize::MAX, usize::MAX)).len(),
        record.len()
    );
}

#[test]
fn a_compact_ledger_from_an_older_writer_is_read_and_rewritten_fixed_width() {
    let _home = HomeGuard::new();
    let dir = session_trace_dir("compact-ledger");
    let ledger = dir.join(LEDGER);
    crate::refs::write_private_file(&ledger, b"[10,1,2]").unwrap();
    {
        let mut budget = TraceWriteBudget::acquire(&dir).unwrap();
        assert_eq!(budget.usage, (10, 1, 3));
        budget.reserve_line(5).unwrap();
    }
    let usage: (usize, usize, usize) =
        serde_json::from_slice(&std::fs::read(&ledger).unwrap()).unwrap();
    assert_eq!(usage, (15, 2, 3));
}

#[test]
fn a_fresh_session_counts_the_ledger_entry_once() {
    let _home = HomeGuard::new();
    let dir = session_trace_dir("fresh-entries");
    let budget = TraceWriteBudget::acquire(&dir).unwrap();
    assert_eq!(budget.usage, (0, 0, 3));
}

#[test]
fn an_empty_ledger_left_by_a_killed_writer_is_rebuilt_without_counting_it_twice() {
    let _home = HomeGuard::new();
    let dir = session_trace_dir("empty-ledger");
    drop(open_private_lock(&dir.join(LEDGER), true).unwrap());
    let budget = TraceWriteBudget::acquire(&dir).unwrap();
    assert_eq!(budget.usage, (0, 0, 3));
}

#[test]
fn a_ledger_longer_than_the_record_is_emptied_before_the_rewrite() {
    let _home = HomeGuard::new();
    let dir = session_trace_dir("long-ledger");
    let ledger = dir.join(LEDGER);
    let mut padded = b"[10,1,2]".to_vec();
    padded.resize(100, b' ');
    crate::refs::write_private_file(&ledger, &padded).unwrap();
    drop(TraceWriteBudget::acquire(&dir).unwrap());
    let record = std::fs::read(&ledger).unwrap();
    assert_eq!(record.len(), encode_ledger((0, 0, 0)).len());
    let usage: (usize, usize, usize) = serde_json::from_slice(&record).unwrap();
    assert_eq!(usage, (10, 1, 3));
}
