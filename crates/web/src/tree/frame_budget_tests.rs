use std::time::Duration;

use agent_desktop_core::{AdapterError, Deadline};

use super::{iframe_deadline, missed_budget};
use crate::transport::transport_error::transport_timeout;

#[test]
fn iframe_reads_stop_a_fifth_short_of_the_snapshot_deadline() {
    let deadline = Deadline::after(3_000).unwrap();

    let budget = iframe_deadline(deadline).remaining();

    assert!(budget <= Duration::from_millis(2_400), "{budget:?}");
    assert!(budget >= Duration::from_millis(2_300), "{budget:?}");
}

#[test]
fn a_short_deadline_still_keeps_the_minimum_reserve() {
    let deadline = Deadline::after(300).unwrap();

    let budget = iframe_deadline(deadline).remaining();

    assert!(budget <= Duration::from_millis(200), "{budget:?}");
}

#[test]
fn a_deadline_inside_the_reserve_leaves_iframes_no_time() {
    let deadline = Deadline::after(50).unwrap();

    assert!(iframe_deadline(deadline).is_expired());
}

#[test]
fn only_a_timeout_with_snapshot_time_left_is_a_missed_budget() {
    let live = Deadline::after(1_000).unwrap();
    let spent = Deadline::after(1).unwrap();
    std::thread::sleep(Duration::from_millis(5));

    assert!(missed_budget(&transport_timeout("receive"), live));
    assert!(!missed_budget(&transport_timeout("receive"), spent));
    assert!(!missed_budget(
        &AdapterError::not_supported("getFullAXTree"),
        live
    ));
}
