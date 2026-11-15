use super::{ScrollSignals, settle};
use agent_desktop_core::ScrollPage;

fn signals(bar: u64) -> ScrollSignals {
    ScrollSignals {
        bar: Some(Some(bar)),
        content: Some(Some(7)),
        descendant: None,
        sampled: 2,
    }
}

#[test]
fn a_signal_that_moves_settles_the_page_as_moved() {
    let mut reads = vec![signals(2), signals(1)].into_iter();
    let page = settle(signals(1), || reads.next().unwrap_or(signals(1)), || true);
    assert_eq!(page, ScrollPage::Moved);
}

#[test]
fn signals_unchanged_through_every_read_are_the_end_of_the_area() {
    let page = settle(signals(1), || signals(1), || true);
    assert_eq!(page, ScrollPage::AtEnd { delivered: true });
}

#[test]
fn unreadable_signals_leave_the_page_unconfirmed() {
    let unreadable = ScrollSignals {
        bar: Some(None),
        content: Some(None),
        descendant: None,
        sampled: 0,
    };
    let page = settle(signals(1), || unreadable, || true);
    assert_eq!(page, ScrollPage::Unconfirmed);
}

#[test]
fn a_spent_deadline_ends_the_reads_without_claiming_an_end() {
    let unreadable = ScrollSignals {
        bar: Some(None),
        content: None,
        descendant: None,
        sampled: 0,
    };
    let page = settle(signals(1), || unreadable, || false);
    assert_eq!(page, ScrollPage::Unconfirmed);
}
