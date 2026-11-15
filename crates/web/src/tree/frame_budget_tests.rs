use std::time::Duration;

use agent_desktop_core::{AdapterError, Deadline};

use super::FrameBudget;
use crate::transport::transport_error::transport_timeout;

#[test]
fn iframe_reads_stop_a_fifth_short_of_the_snapshot_deadline() {
    let budget = FrameBudget::reserved(Deadline::after(3_000).unwrap());

    let remaining = budget.iframes().remaining();

    assert!(remaining <= Duration::from_millis(2_400), "{remaining:?}");
    assert!(remaining >= Duration::from_millis(2_300), "{remaining:?}");
}

#[test]
fn a_short_deadline_still_keeps_the_minimum_reserve() {
    let budget = FrameBudget::reserved(Deadline::after(300).unwrap());

    assert!(budget.iframes().remaining() <= Duration::from_millis(200));
}

#[test]
fn a_deadline_inside_the_reserve_leaves_iframes_no_time() {
    assert!(FrameBudget::reserved(Deadline::after(50).unwrap()).is_spent());
}

#[test]
fn only_a_timeout_with_snapshot_time_left_is_a_missed_budget() {
    let live = FrameBudget::reserved(Deadline::after(1_000).unwrap());
    let spent = FrameBudget::reserved(Deadline::after(1).unwrap());
    std::thread::sleep(Duration::from_millis(5));

    assert!(live.missed(&transport_timeout("receive")));
    assert!(!spent.missed(&transport_timeout("receive")));
    assert!(!live.missed(&AdapterError::not_supported("getFullAXTree")));
}

#[test]
fn a_whole_deadline_read_never_leaves_an_iframe_out() {
    let whole = FrameBudget::whole(Deadline::after(50).unwrap());

    assert!(!whole.missed(&transport_timeout("receive")));
    assert!(!whole.is_spent());
}
