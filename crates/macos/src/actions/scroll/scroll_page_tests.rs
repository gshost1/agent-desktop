use std::cell::Cell;

use accessibility_sys::{kAXErrorCannotComplete, kAXErrorFailure};
use agent_desktop_core::{AdapterError, DeliveryDisposition, ErrorCode, Rect, RetryDisposition};

use super::settle_observed;

fn frame(y: f64) -> Rect {
    Rect {
        x: 40.0,
        y,
        width: 980.0,
        height: 1400.0,
    }
}

fn perform_error(ax_error: i32) -> AdapterError {
    crate::actions::ax_mutation::classify_perform("AXScrollLeftByPage", ax_error).unwrap_err()
}

#[test]
fn refused_page_action_that_moved_nothing_is_an_unavailable_action() {
    let reads = Cell::new(0);
    let error = settle_observed(
        "AXScrollLeftByPage",
        Some(frame(88.0)),
        perform_error(kAXErrorCannotComplete),
        || {
            reads.set(reads.get() + 1);
            Some(frame(88.0))
        },
        || true,
    )
    .unwrap_err();

    assert_eq!(reads.get(), super::SETTLE_READS);
    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    assert_ne!(error.code, ErrorCode::AppUnresponsive);
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
fn refused_page_action_whose_content_moved_counts_as_the_scroll() {
    let reads = Cell::new(0);
    let settled = settle_observed(
        "AXScrollDownByPage",
        Some(frame(88.0)),
        perform_error(kAXErrorCannotComplete),
        || {
            reads.set(reads.get() + 1);
            Some(frame(if reads.get() == 1 { 88.0 } else { -540.0 }))
        },
        || true,
    );

    assert!(settled.is_ok());
    assert_eq!(reads.get(), 2);
}

#[test]
fn refusal_without_a_readable_frame_keeps_the_uncertain_error() {
    for (before, after) in [(None, Some(frame(88.0))), (Some(frame(88.0)), None)] {
        let error = settle_observed(
            "AXScrollLeftByPage",
            before,
            perform_error(kAXErrorCannotComplete),
            || after,
            || true,
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::AppUnresponsive);
        assert_eq!(
            error.disposition.delivery(),
            DeliveryDisposition::DeliveryUncertain
        );
    }
}

#[test]
fn other_page_action_failures_and_expired_settling_are_not_reclassified() {
    let error = settle_observed(
        "AXScrollLeftByPage",
        Some(frame(88.0)),
        perform_error(kAXErrorFailure),
        || Some(frame(88.0)),
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
        Some(frame(88.0)),
        perform_error(kAXErrorCannotComplete),
        || Some(frame(88.0)),
        || false,
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::AppUnresponsive);
}
