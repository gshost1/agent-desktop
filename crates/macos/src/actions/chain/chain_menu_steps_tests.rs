use agent_desktop_core::{
    ActionStep, AdapterError, DeliveryDisposition, DeliverySemantics, RetryDisposition,
};

use super::{after_selection, may_open_menu, selection_after_write};
use crate::actions::chain::chain_delivery::DeliveryOutcome;
use crate::actions::chain::menu_observation::MenuObservation;
use crate::actions::chain::selection_write::SelectionWrite;

fn readback_timeout() -> AdapterError {
    AdapterError::timeout("AXSelected readback did not finish")
}

fn never_run() -> Result<DeliveryOutcome, AdapterError> {
    panic!("nothing may run after a selection that was not written")
}

#[test]
fn only_a_menu_read_as_closed_lets_a_step_open_one() {
    assert!(may_open_menu(MenuObservation::Closed));
    assert!(!may_open_menu(MenuObservation::Open));
    assert!(!may_open_menu(MenuObservation::Unreadable));
}

#[test]
fn a_failed_readback_after_a_selection_write_is_delivered_and_unsafe_to_retry() {
    let error = selection_after_write(|| Err(readback_timeout())).expect_err("the readback failed");
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert_eq!(error.disposition.retry(), RetryDisposition::Unsafe);
}

#[test]
fn a_readback_failure_keeps_its_delivery_through_the_chain() {
    let skipped = [ActionStep::skipped("show_menu")];
    let error = selection_after_write(|| Err(readback_timeout())).expect_err("the readback failed");
    let reported = crate::actions::chain::after_steps(&skipped, error);
    assert_eq!(
        reported.disposition.delivery(),
        DeliveryDisposition::DeliveredUnverified
    );
    assert_eq!(reported.disposition.retry(), RetryDisposition::Unsafe);
}

#[test]
fn a_readback_names_whether_the_written_selection_held() {
    assert_eq!(
        selection_after_write(|| Ok(true)).unwrap(),
        SelectionWrite::Selected
    );
    assert_eq!(
        selection_after_write(|| Ok(false)).unwrap(),
        SelectionWrite::Unconfirmed
    );
}

#[test]
fn a_step_that_selected_and_then_failed_reports_the_selection_as_delivered() {
    let skipped = [ActionStep::skipped("show_menu")];
    let error = after_selection(SelectionWrite::Selected, || Err(readback_timeout()))
        .expect_err("the menu step failed");
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let reported = crate::actions::chain::after_steps(&skipped, error);
    assert_eq!(reported.disposition.retry(), RetryDisposition::Unsafe);
}

#[test]
fn a_step_that_selected_but_opened_no_menu_still_delivered_the_selection() {
    for outcome in [
        DeliveryOutcome::NotDelivered,
        DeliveryOutcome::SatisfiedNoDelivery,
    ] {
        assert_eq!(
            after_selection(SelectionWrite::Selected, || Ok(outcome)).unwrap(),
            DeliveryOutcome::DeliveredUnverified
        );
    }
    assert_eq!(
        after_selection(SelectionWrite::Unconfirmed, never_run).unwrap(),
        DeliveryOutcome::DeliveredUnverified
    );
}

#[test]
fn a_delivered_menu_after_a_selection_keeps_its_own_outcome() {
    for outcome in [
        DeliveryOutcome::DeliveredUnverified,
        DeliveryOutcome::DeliveredVerified,
    ] {
        assert_eq!(
            after_selection(SelectionWrite::Selected, || Ok(outcome)).unwrap(),
            outcome
        );
    }
}

#[test]
fn a_selection_that_was_never_written_delivers_nothing() {
    assert_eq!(
        after_selection(SelectionWrite::NotWritten, never_run).unwrap(),
        DeliveryOutcome::NotDelivered
    );
}
