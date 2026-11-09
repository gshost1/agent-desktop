use std::time::Duration;

use super::*;
use crate::wait::deadline::VirtualClock;

fn remaining(budget: CloseBudget, phase: ClosePhase) -> Duration {
    budget.deadline(phase).remaining()
}

#[test]
fn default_budget_reserves_half_for_the_close_and_a_slice_for_the_confirm_watch() {
    let _clock = VirtualClock::freeze();
    let budget = CloseBudget::split(Deadline::after(5_000).unwrap());

    assert_eq!(
        remaining(budget, ClosePhase::Resolving),
        Duration::from_millis(2_500)
    );
    assert_eq!(
        remaining(budget, ClosePhase::AwaitingLease),
        Duration::from_millis(2_500)
    );
    assert_eq!(
        remaining(budget, ClosePhase::Closing),
        Duration::from_millis(4_750)
    );
    assert_eq!(
        remaining(budget, ClosePhase::ConfirmingExit),
        Duration::from_millis(5_000)
    );
}

#[test]
fn short_budget_keeps_the_minimum_confirm_slice() {
    let _clock = VirtualClock::freeze();
    let budget = CloseBudget::split(Deadline::after(400).unwrap());

    assert_eq!(
        remaining(budget, ClosePhase::AwaitingLease),
        Duration::from_millis(200)
    );
    assert_eq!(
        remaining(budget, ClosePhase::Revalidating),
        Duration::from_millis(350)
    );
}

#[test]
fn tiny_budget_still_confirms_and_never_gives_the_close_the_whole_deadline() {
    let _clock = VirtualClock::freeze();
    let budget = CloseBudget::split(Deadline::after(100).unwrap());

    assert_eq!(
        remaining(budget, ClosePhase::AwaitingLease),
        Duration::from_millis(50)
    );
    assert_eq!(
        remaining(budget, ClosePhase::Closing),
        Duration::from_millis(75)
    );
    assert_eq!(
        remaining(budget, ClosePhase::ConfirmingExit),
        Duration::from_millis(100)
    );
}

#[test]
fn spent_lease_wait_leaves_the_close_its_reserve() {
    let clock = VirtualClock::freeze();
    let budget = CloseBudget::split(Deadline::after(5_000).unwrap());

    clock.advance(remaining(budget, ClosePhase::AwaitingLease));

    assert!(budget.deadline(ClosePhase::AwaitingLease).is_expired());
    assert_eq!(
        remaining(budget, ClosePhase::Closing),
        Duration::from_millis(2_250)
    );
}

#[test]
fn failure_before_the_request_is_not_delivered_and_names_its_slice() {
    let _clock = VirtualClock::freeze();
    let budget = CloseBudget::split(Deadline::after(5_000).unwrap());
    let timeout = budget.deadline(ClosePhase::AwaitingLease).timeout_error();

    let error = budget
        .fail(ClosePhase::AwaitingLease, timeout)
        .into_adapter_error();

    assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
    let details = error.details.unwrap();
    assert_eq!(details["phase"], "awaiting_lease");
    assert_eq!(details["timeout_ms"], 2_500);
    assert_eq!(details["kind"], "deadline");
}

#[test]
fn closing_failure_keeps_its_own_delivery() {
    let _clock = VirtualClock::freeze();
    let budget = CloseBudget::split(Deadline::after(5_000).unwrap());
    let timeout = budget
        .deadline(ClosePhase::Closing)
        .timeout_error()
        .with_disposition(crate::DeliverySemantics::delivered_unverified());

    let error = budget
        .fail(ClosePhase::Closing, timeout)
        .into_adapter_error();

    assert_eq!(
        error.disposition,
        crate::DeliverySemantics::delivered_unverified()
    );
    assert_eq!(error.details.unwrap()["timeout_ms"], 4_750);
}
