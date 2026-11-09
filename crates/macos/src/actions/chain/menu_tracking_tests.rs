use accessibility_sys::{kAXErrorCannotComplete, kAXErrorFailure};
use agent_desktop_core::{DeliveryDisposition, ErrorCode, RetryDisposition};

use super::{is_tracking_timeout, settle_tracking_timeout};
use crate::actions::chain::chain_delivery::DeliveryOutcome;
use crate::actions::chain::menu_observation::MenuObservation;

fn show_menu_timeout() -> agent_desktop_core::AdapterError {
    crate::actions::ax_mutation::classify_perform("AXShowMenu", kAXErrorCannotComplete)
        .expect_err("kAXErrorCannotComplete is a failure code")
}

fn observer(script: &[MenuObservation]) -> impl FnMut() -> Option<MenuObservation> + '_ {
    let mut reads = script.iter().copied();
    move || reads.next()
}

#[test]
fn a_timed_out_show_menu_that_opened_a_menu_is_a_verified_delivery() {
    let outcome = settle_tracking_timeout(
        show_menu_timeout(),
        MenuObservation::Closed,
        observer(&[MenuObservation::Closed, MenuObservation::Open]),
    )
    .expect("an observed menu is the postcondition");

    assert_eq!(outcome, DeliveryOutcome::DeliveredVerified);
    assert!(outcome.was_delivered());
    assert!(outcome.was_verified());
}

#[test]
fn a_responsive_application_without_a_menu_is_not_reported_as_delivered_or_hung() {
    let error = settle_tracking_timeout(
        show_menu_timeout(),
        MenuObservation::Closed,
        observer(&[MenuObservation::Closed; 4]),
    )
    .expect_err("no menu means the action is not proven");

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::DeliveryUncertain
    );
    assert_eq!(error.disposition.retry(), RetryDisposition::Unsafe);
    let details = error.details.expect("details name the timed-out action");
    assert_eq!(details["operation"], "AXShowMenu");
    assert_eq!(details["menu_observed"], false);
    assert_eq!(details["perform_budget_ms"], super::MENU_ACTION_BUDGET_MS);
    assert_eq!(details["ax_error"], kAXErrorCannotComplete);
    assert!(error.platform_detail.is_some());
}

#[test]
fn an_application_that_never_answers_keeps_its_unresponsive_failure() {
    let error = settle_tracking_timeout(
        show_menu_timeout(),
        MenuObservation::Closed,
        observer(&[MenuObservation::Unreadable; 4]),
    )
    .expect_err("a hang never proves delivery");

    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::DeliveryUncertain
    );
    assert_eq!(error.disposition.retry(), RetryDisposition::Unsafe);
}

#[test]
fn a_menu_without_a_closed_baseline_does_not_verify_the_delivery() {
    for baseline in [MenuObservation::Unreadable, MenuObservation::Open] {
        let error = settle_tracking_timeout(
            show_menu_timeout(),
            baseline,
            observer(&[MenuObservation::Open]),
        )
        .expect_err("the menu may predate the action");

        assert_eq!(error.code, ErrorCode::AppUnresponsive);
        assert_eq!(
            error.disposition.delivery(),
            DeliveryDisposition::DeliveryUncertain
        );
    }
}

#[test]
fn an_exhausted_observation_budget_keeps_the_original_failure() {
    let error = settle_tracking_timeout(show_menu_timeout(), MenuObservation::Closed, || None)
        .expect_err("no read is no evidence");

    assert_eq!(error.code, ErrorCode::AppUnresponsive);
}

#[test]
fn only_a_cannot_complete_perform_is_settled_by_a_menu_read() {
    assert!(is_tracking_timeout(&show_menu_timeout()));

    let generic = crate::actions::ax_mutation::classify_perform("AXShowMenu", kAXErrorFailure)
        .expect_err("kAXErrorFailure is a failure code");
    assert!(!is_tracking_timeout(&generic));
    assert!(!is_tracking_timeout(
        &agent_desktop_core::AdapterError::new(ErrorCode::AppUnresponsive, "a read timed out",)
    ));
}

#[test]
fn a_menu_opening_perform_never_waits_out_the_callers_whole_deadline() {
    let long = agent_desktop_core::Deadline::after(10_000).expect("deadline");
    let call = super::menu_action_deadline(long);
    assert!(call.remaining() <= std::time::Duration::from_millis(500));
    assert!(!call.is_expired());

    let short = agent_desktop_core::Deadline::after(100).expect("deadline");
    assert!(
        super::menu_action_deadline(short).remaining() <= std::time::Duration::from_millis(100)
    );
}

#[test]
fn the_menu_read_after_a_timeout_outlasts_the_perform_it_settles() {
    const { assert!(super::TRACKING_SETTLE_MS > super::MENU_ACTION_BUDGET_MS) };
}
