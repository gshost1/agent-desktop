use super::{KeyboardFocus, establish_with, set_focus_refusal};
use crate::system::runtime::hresult::{
    UIA_E_ELEMENTNOTAVAILABLE, UIA_E_NOTSUPPORTED, UIA_E_TIMEOUT,
};
use crate::tree::automation::UiaFailure;
use agent_desktop_core::{
    ActionStep, AdapterError, Deadline, DeliveryDisposition, ErrorCode, InteractionPolicy,
    StepMechanism,
};
use std::cell::Cell;
use std::time::{Duration, Instant};

fn deadline(ms: u64) -> Deadline {
    Deadline::after(ms).expect("deadline")
}

fn keystroke() -> ActionStep {
    ActionStep::succeeded("SendInput.type_text").with_mechanism(StepMechanism::PhysicalSynthetic)
}

#[test]
fn a_target_already_holding_focus_is_not_refocused() {
    let asked = Cell::new(false);

    let focus = establish_with(
        deadline(5_000),
        InteractionPolicy::headed(),
        || {
            asked.set(true);
            Ok(())
        },
        |_| Ok(true),
    )
    .expect("focus");

    assert_eq!(focus, KeyboardFocus::AlreadyFocused);
    assert!(!asked.get());
    assert!(focus.is_ready());
    let steps = focus.then(keystroke());
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].label(), "SendInput.type_text");
}

#[test]
fn a_headed_keystroke_focuses_an_unfocused_target_first() {
    let asked = Cell::new(false);
    let reads = Cell::new(0);

    let focus = establish_with(
        deadline(5_000),
        InteractionPolicy::headed(),
        || {
            asked.set(true);
            Ok(())
        },
        |_| {
            reads.set(reads.get() + 1);
            Ok(asked.get() && reads.get() >= 3)
        },
    )
    .expect("focus");

    assert_eq!(focus, KeyboardFocus::FocusedBySetFocus);
    assert!(focus.is_ready());
    let steps = focus.then(keystroke());
    let labels: Vec<&str> = steps.iter().map(|step| step.label()).collect();
    assert_eq!(labels, ["UIElement.SetFocus", "SendInput.type_text"]);
    assert_eq!(steps[0].mechanism(), Some(StepMechanism::SemanticApi));
    assert_eq!(steps[0].verified(), Some(true));
}

#[test]
fn a_focus_fallback_keystroke_never_moves_focus() {
    let asked = Cell::new(false);

    let focus = establish_with(
        deadline(5_000),
        InteractionPolicy::focus_fallback(),
        || {
            asked.set(true);
            Ok(())
        },
        |_| Ok(false),
    )
    .expect("focus");

    assert_eq!(focus, KeyboardFocus::NotFocused);
    assert!(!asked.get());
    assert!(!focus.is_ready());
}

#[test]
fn a_headless_keystroke_never_moves_focus() {
    let asked = Cell::new(false);

    let focus = establish_with(
        deadline(5_000),
        InteractionPolicy::headless(),
        || {
            asked.set(true);
            Ok(())
        },
        |_| Ok(false),
    )
    .expect("focus");

    assert_eq!(focus, KeyboardFocus::NotFocused);
    assert!(!asked.get());
}

#[test]
fn a_set_focus_refusal_is_returned_as_it_is() {
    let error = establish_with(
        deadline(5_000),
        InteractionPolicy::headed(),
        || Err(AdapterError::new(ErrorCode::StaleRef, "element gone")),
        |_| Ok(false),
    )
    .expect_err("refusal");

    assert_eq!(error.code, ErrorCode::StaleRef);
}

#[test]
fn focus_that_never_lands_is_refused_after_the_settle_window() {
    let started = Instant::now();

    let error = establish_with(
        deadline(10_000),
        InteractionPolicy::headed(),
        || Ok(()),
        |_| Ok(false),
    )
    .expect_err("never focused");

    assert!(started.elapsed() >= Duration::from_millis(400));
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.message, "Target did not take keyboard focus");
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
}

/// A slower host can only make fewer focus reads, so a ceiling on the count
/// proves the settle window was cut to the deadline without a wall-clock
/// ceiling that a loaded runner could break.
#[test]
fn the_settle_window_is_cut_to_a_shorter_deadline() {
    let reads = Cell::new(0);

    let error = establish_with(
        deadline(120),
        InteractionPolicy::headed(),
        || Ok(()),
        |_| {
            reads.set(reads.get() + 1);
            Ok(false)
        },
    )
    .expect_err("never focused");

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert!(reads.get() <= 10, "{} focus reads", reads.get());
}

#[test]
fn a_failed_focus_read_is_returned_without_moving_focus() {
    let asked = Cell::new(false);

    let error = establish_with(
        deadline(5_000),
        InteractionPolicy::headed(),
        || {
            asked.set(true);
            Ok(())
        },
        |_| Err(AdapterError::new(ErrorCode::StaleRef, "owner exited")),
    )
    .expect_err("read error");

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert!(!asked.get());
}

#[test]
fn a_set_focus_on_a_vanished_element_is_a_stale_ref() {
    let error = set_focus_refusal(&UiaFailure::Hresult(UIA_E_ELEMENTNOTAVAILABLE));

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
    assert!(error.platform_detail.is_some());
}

#[test]
fn a_target_without_a_focus_affordance_did_not_take_focus() {
    let error = set_focus_refusal(&UiaFailure::Hresult(UIA_E_NOTSUPPORTED));

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.message, "Target did not take keyboard focus");
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
    let detail = error.platform_detail.expect("hresult detail");
    assert!(detail.contains("UIElement.SetFocus"), "{detail}");
}

#[test]
fn an_uncertain_set_focus_failure_is_refused_as_focus_not_taken() {
    let error = set_focus_refusal(&UiaFailure::Hresult(UIA_E_TIMEOUT));

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.message, "Target did not take keyboard focus");
    assert!(!error.message.contains("uncertain"));
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
    assert!(error.platform_detail.is_some());
}
