use std::cell::Cell;
use std::time::Duration;

use accessibility_sys::{kAXErrorCannotComplete, kAXErrorFailure};
use agent_desktop_core::{AdapterError, DeliveryDisposition, ErrorCode, RetryDisposition};

use super::{answer_window, answered_in_time, settle_observed};
use crate::actions::scroll::scroll_signals::ScrollSignals;

fn signals(
    bar: Option<Option<u64>>,
    content: Option<Option<u64>>,
    descendant: Option<Option<u64>>,
) -> ScrollSignals {
    let sampled = [content, descendant]
        .iter()
        .filter(|signal| matches!(signal, Some(Some(_))))
        .count();
    ScrollSignals {
        bar,
        content,
        descendant,
        sampled,
    }
}

fn steady() -> ScrollSignals {
    signals(
        Some(Some(0.25f64.to_bits())),
        Some(Some(88)),
        Some(Some(120)),
    )
}

fn perform_error(ax_error: i32) -> AdapterError {
    crate::actions::ax_mutation::classify_perform("AXScrollLeftByPage", ax_error).unwrap_err()
}

fn settle(
    before: ScrollSignals,
    after: impl Fn(usize) -> ScrollSignals,
) -> (Result<(), AdapterError>, usize) {
    let reads = Cell::new(0);
    let settled = settle_observed(
        "AXScrollDownByPage",
        before,
        perform_error(kAXErrorCannotComplete),
        || {
            reads.set(reads.get() + 1);
            after(reads.get())
        },
        || true,
    );
    (settled, reads.get())
}

fn assert_uncertain(error: &AdapterError) {
    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::DeliveryUncertain
    );
}

#[test]
fn refused_page_action_that_moved_nothing_is_an_unavailable_action() {
    let (settled, reads) = settle(steady(), |_| steady());
    let error = settled.unwrap_err();

    assert_eq!(reads, super::SETTLE_READS);
    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
    assert_eq!(error.disposition.retry(), RetryDisposition::Safe);
    assert_eq!(
        error.details.as_ref().unwrap()["kind"],
        "scroll_action_unavailable"
    );
}

#[test]
fn settle_window_is_long_enough_for_an_animated_scroll_to_start() {
    let window = super::SETTLE_PAUSE * (super::SETTLE_READS as u32 - 1);
    assert!(window >= std::time::Duration::from_millis(250));
}

#[test]
fn scroll_bar_that_moved_while_the_content_frame_stayed_counts_as_the_scroll() {
    let (settled, reads) = settle(steady(), |read| {
        if read < 4 {
            steady()
        } else {
            signals(
                Some(Some(0.6f64.to_bits())),
                Some(Some(88)),
                Some(Some(120)),
            )
        }
    });

    assert!(settled.is_ok());
    assert_eq!(reads, 4);
}

#[test]
fn descendant_that_moved_inside_a_fixed_content_frame_counts_as_the_scroll() {
    let before = signals(None, Some(Some(88)), Some(Some(120)));
    let (settled, _) = settle(before, |read| {
        signals(
            None,
            Some(Some(88)),
            Some(Some(if read < 6 { 120 } else { 977 })),
        )
    });

    assert!(settled.is_ok());
}

#[test]
fn signal_that_becomes_unreadable_keeps_the_uncertain_error() {
    for after in [
        signals(Some(None), Some(Some(88)), Some(Some(120))),
        signals(Some(Some(0.25f64.to_bits())), Some(None), Some(Some(120))),
        signals(Some(Some(0.25f64.to_bits())), Some(Some(88)), None),
    ] {
        let (settled, _) = settle(steady(), |_| after);
        assert_uncertain(&settled.unwrap_err());
    }
}

#[test]
fn area_without_a_scroll_bar_whose_content_is_unreadable_keeps_the_uncertain_error() {
    let unreadable = signals(None, Some(None), Some(None));
    let (settled, reads) = settle(unreadable, |_| unreadable);
    assert_uncertain(&settled.unwrap_err());
    assert_eq!(reads, 1);

    let nothing = ScrollSignals::default();
    let (settled, _) = settle(nothing, |_| nothing);
    assert_uncertain(&settled.unwrap_err());
}

#[test]
fn other_page_action_failures_and_expired_settling_are_not_reclassified() {
    let error = settle_observed(
        "AXScrollLeftByPage",
        steady(),
        perform_error(kAXErrorFailure),
        steady,
        || true,
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::DeliveryUncertain
    );

    let error = settle_observed(
        "AXScrollLeftByPage",
        steady(),
        perform_error(kAXErrorCannotComplete),
        steady,
        || false,
    )
    .unwrap_err();
    assert_uncertain(&error);
}

#[test]
fn a_single_unchanged_frame_without_a_scroll_bar_is_no_proof() {
    let lone_frame = signals(None, Some(Some(88)), None);
    let (settled, reads) = settle(lone_frame, |_| lone_frame);
    assert_uncertain(&settled.unwrap_err());
    assert_eq!(
        reads, 1,
        "a content frame sized to the viewport never moves, so one frame cannot show the \
         scroll did not happen"
    );

    let two_frames = ScrollSignals {
        sampled: 2,
        ..lone_frame
    };
    let (settled, _) = settle(two_frames, |_| two_frames);
    assert_eq!(settled.unwrap_err().code, ErrorCode::ActionNotSupported);
}

#[test]
fn a_moved_row_behind_a_pinned_header_counts_as_the_scroll() {
    let before = ScrollSignals {
        sampled: 3,
        ..signals(None, Some(Some(88)), Some(Some(120)))
    };
    let (settled, _) = settle(before, |read| ScrollSignals {
        descendant: Some(Some(if read < 3 { 120 } else { 121 })),
        ..before
    });

    assert!(
        settled.is_ok(),
        "the descendant signal hashes several children, so a row that moved under a fixed \
         header changes it"
    );
}

#[test]
fn a_refusal_that_ran_into_its_messaging_timeout_is_not_settled() {
    let window = answer_window(agent_desktop_core::Deadline::after(2_000).unwrap());

    assert!(window > Duration::from_millis(400) && window <= Duration::from_millis(500));
    assert!(answered_in_time(Duration::from_millis(3), window));
    assert!(
        !answered_in_time(Duration::from_millis(1_000), window),
        "a call that waited out its messaging timeout reached the application, which may \
         still scroll after the settle window, so its delivery stays unknown"
    );
}
