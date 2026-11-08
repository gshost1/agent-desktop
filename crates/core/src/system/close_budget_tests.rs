use std::time::Duration;

use super::*;
use crate::wait::deadline::VirtualClock;

#[test]
fn default_budget_reserves_half_for_the_close_and_a_slice_for_the_confirm_read() {
    let _clock = VirtualClock::freeze();
    let budget = CloseBudget::split(Deadline::after(5_000).unwrap());

    assert_eq!(
        budget.deadline(ClosePhase::Resolving).remaining(),
        Duration::from_millis(2_500)
    );
    assert_eq!(
        budget.deadline(ClosePhase::AwaitingLease).remaining(),
        Duration::from_millis(2_500)
    );
    assert_eq!(
        budget.deadline(ClosePhase::Closing).remaining(),
        Duration::from_millis(4_750)
    );
    assert_eq!(
        budget.deadline(ClosePhase::ConfirmingExit).remaining(),
        Duration::from_millis(5_000)
    );
}

#[test]
fn short_budget_scales_the_confirm_slice_down() {
    let _clock = VirtualClock::freeze();
    let budget = CloseBudget::split(Deadline::after(400).unwrap());

    assert_eq!(
        budget.deadline(ClosePhase::AwaitingLease).remaining(),
        Duration::from_millis(200)
    );
    assert_eq!(
        budget.deadline(ClosePhase::Revalidating).remaining(),
        Duration::from_millis(350)
    );
}

#[test]
fn spent_lease_wait_leaves_the_close_its_reserve() {
    let clock = VirtualClock::freeze();
    let budget = CloseBudget::split(Deadline::after(5_000).unwrap());

    clock.advance(budget.deadline(ClosePhase::AwaitingLease).remaining());

    assert!(budget.deadline(ClosePhase::AwaitingLease).is_expired());
    assert_eq!(
        budget.deadline(ClosePhase::Closing).remaining(),
        Duration::from_millis(2_250)
    );
}
