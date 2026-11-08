use std::time::Instant;

use super::*;
use crate::system::window::window_op::window_op_verdict::{POLL, SETTLE_BUDGET};

const LATE_CONFIRMATION: Duration = Duration::from_millis(800);

fn before() -> WindowFacts {
    state(10, 10, 400, 300)
}

fn asked() -> WindowFacts {
    state(300, 200, 400, 300)
}

fn since_the_write() -> Duration {
    thread_local! {
        static WRITTEN_AT: Cell<Option<Instant>> = const { Cell::new(None) };
    }
    WRITTEN_AT.with(|at| {
        let since = at.get().unwrap_or_else(Instant::now);
        at.set(Some(since));
        since.elapsed()
    })
}

fn wait_until(after: Duration) {
    std::thread::sleep(after.saturating_sub(since_the_write()));
}

fn counted_read() -> u32 {
    thread_local! {
        static READS: Cell<u32> = const { Cell::new(0) };
    }
    READS.with(|reads| reads.replace(reads.get() + 1))
}

fn landing_after(written: &[WindowWrite], after: Duration) -> ControlRead<WindowFacts> {
    if !written.is_empty() && since_the_write() >= after {
        ControlRead::Ready(asked())
    } else {
        ControlRead::Ready(before())
    }
}

fn lands_after_most_of_the_settle_budget(written: &[WindowWrite]) -> ControlRead<WindowFacts> {
    landing_after(written, Duration::from_millis(150))
}

fn lands_just_after_the_settle_budget(written: &[WindowWrite]) -> ControlRead<WindowFacts> {
    landing_after(written, SETTLE_BUDGET + Duration::from_millis(10))
}

fn lands_well_after_the_settle_budget(written: &[WindowWrite]) -> ControlRead<WindowFacts> {
    landing_after(written, SETTLE_BUDGET + Duration::from_millis(500))
}

fn matching_read_that_returns_after_the_settle_budget(
    written: &[WindowWrite],
) -> ControlRead<WindowFacts> {
    if written.is_empty() || since_the_write() < SETTLE_BUDGET - POLL {
        return ControlRead::Ready(before());
    }
    wait_until(SETTLE_BUDGET + POLL);
    ControlRead::Ready(asked())
}

fn after_a_read_across_the_settle_budget(
    written: &[WindowWrite],
    landing_read: u32,
) -> ControlRead<WindowFacts> {
    if written.is_empty() {
        return ControlRead::Ready(before());
    }
    let read = counted_read();
    if read == 0 {
        wait_until(SETTLE_BUDGET + POLL * 2);
    }
    ControlRead::Ready(if read >= landing_read {
        asked()
    } else {
        before()
    })
}

fn lands_on_the_read_after_one_across_the_budget(
    written: &[WindowWrite],
) -> ControlRead<WindowFacts> {
    after_a_read_across_the_settle_budget(written, 1)
}

fn lands_two_reads_after_one_across_the_budget(
    written: &[WindowWrite],
) -> ControlRead<WindowFacts> {
    after_a_read_across_the_settle_budget(written, 2)
}

fn shifts_within_the_tolerance_after_the_budget(
    written: &[WindowWrite],
) -> ControlRead<WindowFacts> {
    if written.is_empty() || since_the_write() < SETTLE_BUDGET - Duration::from_millis(10) {
        return ControlRead::Ready(before());
    }
    ControlRead::Ready(match counted_read() {
        0 => state(301, 200, 400, 300),
        _ => asked(),
    })
}

fn confirmation_returning_after_the_deadline(
    written: &[WindowWrite],
    late: WindowFacts,
) -> ControlRead<WindowFacts> {
    if written.is_empty() {
        return ControlRead::Ready(before());
    }
    ControlRead::Ready(match counted_read() {
        0 => asked(),
        1 => {
            wait_until(LATE_CONFIRMATION);
            late
        }
        _ => late,
    })
}

fn late_and_moved_again(written: &[WindowWrite]) -> ControlRead<WindowFacts> {
    confirmation_returning_after_the_deadline(written, state(60, 90, 400, 300))
}

fn late_and_unchanged(written: &[WindowWrite]) -> ControlRead<WindowFacts> {
    confirmation_returning_after_the_deadline(written, asked())
}

fn unreadable_after_the_match(written: &[WindowWrite]) -> ControlRead<WindowFacts> {
    if written.is_empty() || since_the_write() < Duration::from_millis(150) {
        return ControlRead::Ready(before());
    }
    match counted_read() {
        0 => ControlRead::Ready(asked()),
        _ => ControlRead::Unavailable,
    }
}

fn move_landing(world: &World, deadline_ms: u64) -> Result<OperationReport, AdapterError> {
    run(
        world,
        &window(),
        &WindowOp::Move { x: 300.0, y: 200.0 },
        Deadline::after(deadline_ms).expect("a deadline builds"),
    )
}

fn reacting(reaction: Reaction) -> World {
    let mut world = World::showing(before());
    world.reaction = Some(reaction);
    world
}

#[test]
fn a_place_the_manager_applies_late_in_the_settle_budget_still_gets_its_confirmation() {
    let world = reacting(lands_after_most_of_the_settle_budget);
    move_landing(&world, 2_000)
        .expect("the place matched within the settle budget and held through the pause");
    assert_eq!(world.writes(), 1);
}

#[test]
fn a_place_the_manager_applies_just_after_the_settle_budget_is_seen_by_the_last_look() {
    move_landing(&reacting(lands_just_after_the_settle_budget), 2_000)
        .expect("the last look saw the place and it held through the pause");
}

#[test]
fn a_place_first_seen_well_after_the_settle_budget_is_unverified() {
    let error = move_landing(&reacting(lands_well_after_the_settle_budget), 2_000)
        .expect_err("no read matched before the last look");
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert_eq!(
        error.details.expect("details")["cause"],
        "readback_mismatch"
    );
}

#[test]
fn a_match_read_back_only_after_the_settle_budget_still_gets_its_confirmation() {
    move_landing(
        &reacting(matching_read_that_returns_after_the_settle_budget),
        2_000,
    )
    .expect("a match seen at any read enters its confirmation");
}

#[test]
fn a_read_across_the_settle_budget_is_followed_by_its_last_look() {
    move_landing(
        &reacting(lands_on_the_read_after_one_across_the_budget),
        2_000,
    )
    .expect("the last look after the slow read matched");
}

#[test]
fn a_read_across_the_settle_budget_gets_one_last_look_only() {
    let error = move_landing(
        &reacting(lands_two_reads_after_one_across_the_budget),
        2_000,
    )
    .expect_err("the one last look after the slow read did not match");
    assert_eq!(
        error.details.expect("details")["cause"],
        "readback_mismatch"
    );
}

#[test]
fn a_place_that_shifts_within_the_tolerance_after_the_budget_is_reached_but_unconfirmed() {
    let error = move_landing(
        &reacting(shifts_within_the_tolerance_after_the_budget),
        2_000,
    )
    .expect_err("the place moved within the tolerance during its confirmation");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.expect("details");
    assert_eq!(details["cause"], "readback_unsettled");
    assert_eq!(details["observed"]["x"], 300);
}

#[test]
fn a_late_match_without_room_to_confirm_before_the_deadline_is_unconfirmed() {
    let world = reacting(lands_after_most_of_the_settle_budget);
    let place = super::super::hold::move_to(300, 200);
    world.write(CLIENT, &place).expect("the fake write lands");
    let settled = super::super::settle(
        &world,
        CLIENT,
        &place,
        Deadline::after(250).expect("a deadline builds"),
    );
    assert_eq!(settled, Settled::Unconfirmed(asked()));
}

#[test]
fn a_confirmation_that_returns_after_the_deadline_reports_its_latest_rectangle() {
    let error = move_landing(&reacting(late_and_moved_again), 500)
        .expect_err("a confirmation read past the deadline cannot prove the place held");
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.expect("details");
    assert_eq!(details["observed"]["x"], 60);
    assert_eq!(details["observed"]["y"], 90);
}

#[test]
fn a_confirmation_that_agrees_but_returns_after_the_deadline_is_not_a_held_place() {
    let error = move_landing(&reacting(late_and_unchanged), 500)
        .expect_err("a confirmation read past the deadline cannot prove the place held");
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}

#[test]
fn an_unreadable_confirmation_leaves_the_effect_unknown_and_keeps_the_read_that_matched() {
    let error = move_landing(&reacting(unreadable_after_the_match), 2_000)
        .expect_err("the confirmation could not be read");
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.expect("details");
    assert_eq!(details["cause"], "readback_unavailable");
    assert_eq!(details["observed"]["x"], 300);
}
