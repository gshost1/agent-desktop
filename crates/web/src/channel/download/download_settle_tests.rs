use std::time::Duration;

use super::ledger_deadline;

#[test]
fn the_ledger_lock_bound_outlasts_a_stalled_uncontended_open() {
    let deadline = ledger_deadline().unwrap();

    assert!(deadline.remaining() > Duration::from_millis(1_000));
}
