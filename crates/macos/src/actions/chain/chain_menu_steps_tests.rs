use agent_desktop_core::{
    ActionStep, AdapterError, Deadline, DeliveryDisposition, DeliverySemantics, RetryDisposition,
};

use super::{after_selection, selection_after_write};
use crate::actions::chain::chain_delivery::DeliveryOutcome;
use crate::actions::chain::selection_write::SelectionWrite;

fn short_deadline() -> Deadline {
    Deadline::after(60).unwrap()
}

fn read_timeout() -> AdapterError {
    AdapterError::timeout("menu read did not finish")
}

fn never_run() -> Result<DeliveryOutcome, AdapterError> {
    panic!("nothing may run after a selection that was not written")
}

#[test]
fn a_written_selection_whose_readback_keeps_failing_is_unconfirmed_not_unwritten() {
    assert_eq!(
        selection_after_write(short_deadline(), || Err(read_timeout())),
        SelectionWrite::Unconfirmed
    );
    assert_eq!(
        after_selection(SelectionWrite::Unconfirmed, never_run).unwrap(),
        DeliveryOutcome::DeliveredUnverified
    );
}

#[test]
fn a_selection_that_settles_after_a_late_readback_is_selected() {
    let mut reads = [Ok(false), Ok(false), Ok(true)].into_iter();
    assert_eq!(
        selection_after_write(Deadline::after(1_000).unwrap(), || {
            reads.next().unwrap_or(Ok(true))
        }),
        SelectionWrite::Selected
    );
}

#[test]
fn a_step_that_selected_and_then_failed_reports_the_selection_as_delivered() {
    let skipped = [ActionStep::skipped("show_menu")];
    let error = after_selection(SelectionWrite::Selected, || Err(read_timeout()))
        .expect_err("the menu step failed");
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let reported = crate::actions::chain::after_steps(&skipped, error);
    assert_eq!(
        reported.disposition.delivery(),
        DeliveryDisposition::DeliveredUnverified
    );
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
