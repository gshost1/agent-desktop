use std::cell::RefCell;
use std::collections::VecDeque;

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode, RetryDisposition};

use super::{DismissOps, record, run_dismiss};
use crate::actions::DeliveryTracker;

type Step = Result<bool, AdapterError>;

#[derive(Default)]
struct StubNotification {
    performs: RefCell<VecDeque<Step>>,
    presses: RefCell<VecDeque<Step>>,
    rereads: RefCell<VecDeque<Step>>,
    reveal: Option<AdapterError>,
    calls: RefCell<Vec<String>>,
}

impl StubNotification {
    fn with_performs(performs: Vec<Step>) -> Self {
        Self {
            performs: RefCell::new(performs.into()),
            ..Self::default()
        }
    }

    fn rereading(self, rereads: Vec<Step>) -> Self {
        *self.rereads.borrow_mut() = rereads.into();
        self
    }
}

fn next(queue: &RefCell<VecDeque<Step>>) -> Step {
    queue.borrow_mut().pop_front().unwrap_or(Ok(false))
}

impl DismissOps for StubNotification {
    fn perform(&self, action: &str) -> Result<bool, AdapterError> {
        self.calls.borrow_mut().push(action.to_owned());
        next(&self.performs)
    }

    fn press_close_button(&self, delivery: &mut DeliveryTracker) -> Result<bool, AdapterError> {
        self.calls.borrow_mut().push("press".to_owned());
        let pressed = next(&self.presses);
        record(delivery, pressed.as_ref().copied());
        pressed
    }

    fn is_gone(&self) -> Result<bool, AdapterError> {
        self.calls.borrow_mut().push("reread".to_owned());
        next(&self.rereads)
    }

    fn may_reveal(&self) -> Result<(), AdapterError> {
        self.calls.borrow_mut().push("reveal".to_owned());
        self.reveal.clone().map_or(Ok(()), Err)
    }

    fn hover(&self) -> Result<(), AdapterError> {
        self.calls.borrow_mut().push("hover".to_owned());
        Ok(())
    }
}

fn read_failure(code: ErrorCode) -> AdapterError {
    AdapterError::new(code, "Accessibility read failed")
        .with_disposition(DeliverySemantics::not_delivered())
}

fn dismiss(stub: &StubNotification) -> Result<(), AdapterError> {
    run_dismiss(stub, Deadline::standard().expect("deadline"))
}

#[test]
fn a_permission_failure_on_the_reread_after_a_sent_dismissal_is_not_retry_safe() {
    let stub = StubNotification::with_performs(vec![Ok(true)])
        .rereading(vec![Err(read_failure(ErrorCode::PermDenied))]);

    let error = dismiss(&stub).unwrap_err();

    assert_eq!(error.code, ErrorCode::PermDenied);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert_eq!(error.disposition.retry(), RetryDisposition::Unsafe);
}

#[test]
fn a_later_policy_refusal_keeps_an_earlier_sent_dismissal() {
    let mut stub = StubNotification::with_performs(vec![Ok(true), Ok(false)]);
    stub.reveal = Some(
        AdapterError::new(ErrorCode::PolicyDenied, "pointer not allowed")
            .with_disposition(DeliverySemantics::not_delivered()),
    );

    let error = dismiss(&stub).unwrap_err();

    assert_eq!(error.code, ErrorCode::PolicyDenied);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}

#[test]
fn an_uncertain_native_dismissal_is_counted_even_when_tolerated() {
    let uncertain = DeliveryTracker::uncertain(AdapterError::new(
        ErrorCode::ActionFailed,
        "AXDismiss returned kAXErrorFailure",
    ));
    let stub = StubNotification::with_performs(vec![Err(uncertain), Ok(false)]);

    let error = dismiss(&stub).unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition.retry(), RetryDisposition::Unsafe);
}

#[test]
fn nothing_sent_stays_retry_safe() {
    let stub = StubNotification {
        reveal: Some(
            AdapterError::new(ErrorCode::PolicyDenied, "pointer not allowed")
                .with_disposition(DeliverySemantics::not_delivered()),
        ),
        ..StubNotification::default()
    };

    let error = dismiss(&stub).unwrap_err();

    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(
        *stub.calls.borrow(),
        [
            "AXDismiss",
            "reread",
            "AXRemoveFromParent",
            "reread",
            "press",
            "reread",
            "reveal"
        ]
    );
}

#[test]
fn a_dismissal_proved_by_the_reread_succeeds_without_further_strategies() {
    let stub = StubNotification::with_performs(vec![Ok(true)]).rereading(vec![Ok(true)]);

    dismiss(&stub).expect("verified dismissal");

    assert_eq!(*stub.calls.borrow(), ["AXDismiss", "reread"]);
}

#[test]
fn a_hover_reveal_that_still_fails_is_reported_after_delivery() {
    let stub = StubNotification::default();

    let error = dismiss(&stub).unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert_eq!(
        *stub.calls.borrow(),
        [
            "AXDismiss",
            "reread",
            "AXRemoveFromParent",
            "reread",
            "press",
            "reread",
            "reveal",
            "hover",
            "press",
            "reread"
        ]
    );
}

#[test]
fn only_sent_or_uncertain_outcomes_are_counted() {
    let mut delivery = DeliveryTracker::default();
    record(&mut delivery, Ok(false));
    record(&mut delivery, Err(&read_failure(ErrorCode::StaleRef)));
    assert_eq!(delivery.delivered_units(), 0);

    record(&mut delivery, Ok(true));
    assert_eq!(delivery.delivered_units(), 1);
}
