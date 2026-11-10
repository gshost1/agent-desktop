use agent_desktop_core::{
    ActionStep, AdapterError, Deadline, DeliveryDisposition, DeliverySemantics, RetryDisposition,
};

use super::{after_selection, may_open_menu, selection_after_write, settled_menu_observation};
use crate::actions::chain::chain_delivery::DeliveryOutcome;
use crate::actions::chain::menu_observation::MenuObservation;
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

fn scripted(reads: &[MenuObservation]) -> impl FnMut() -> MenuObservation + '_ {
    let mut reads = reads.iter().copied();
    move || reads.next().unwrap_or(MenuObservation::Unreadable)
}

#[test]
fn only_a_menu_read_as_closed_lets_a_step_open_one() {
    assert!(may_open_menu(MenuObservation::Closed));
    assert!(!may_open_menu(MenuObservation::Open));
    assert!(!may_open_menu(MenuObservation::Unreadable));
}

#[test]
fn a_menu_read_that_stays_unreadable_refuses_the_step() {
    let mut reads = 0;
    let observation = settled_menu_observation(
        || {
            reads += 1;
            MenuObservation::Unreadable
        },
        Deadline::after(1_000).unwrap(),
    );
    assert_eq!(observation, MenuObservation::Unreadable);
    assert!(!may_open_menu(observation));
    assert_eq!(reads, super::MENU_GATE_READS);
}

#[test]
fn one_failed_menu_read_is_read_again_before_it_refuses() {
    let script = [MenuObservation::Unreadable, MenuObservation::Closed];
    assert_eq!(
        settled_menu_observation(scripted(&script), Deadline::after(1_000).unwrap()),
        MenuObservation::Closed
    );
    let script = [MenuObservation::Open, MenuObservation::Closed];
    assert_eq!(
        settled_menu_observation(scripted(&script), Deadline::after(1_000).unwrap()),
        MenuObservation::Open
    );
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
