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

fn landing_after(written: &[WindowWrite], after: Duration) -> WindowFacts {
    if !written.is_empty() && since_the_write() >= after {
        asked()
    } else {
        before()
    }
}

fn lands_after_most_of_the_settle_budget(written: &[WindowWrite]) -> WindowFacts {
    landing_after(written, Duration::from_millis(150))
}

fn lands_just_after_the_settle_budget(written: &[WindowWrite]) -> WindowFacts {
    landing_after(written, SETTLE_BUDGET + Duration::from_millis(10))
}

fn lands_well_after_the_settle_budget(written: &[WindowWrite]) -> WindowFacts {
    landing_after(written, SETTLE_BUDGET + Duration::from_millis(500))
}

fn matching_read_that_returns_after_the_settle_budget(written: &[WindowWrite]) -> WindowFacts {
    if written.is_empty() || since_the_write() < SETTLE_BUDGET - POLL {
        return before();
    }
    wait_until(SETTLE_BUDGET + POLL);
    asked()
}

fn slow_first_read_then_lands(written: &[WindowWrite]) -> WindowFacts {
    if written.is_empty() {
        return before();
    }
    match counted_read() {
        0 => {
            wait_until(SETTLE_BUDGET + POLL * 2);
            before()
        }
        1 => before(),
        _ => asked(),
    }
}

fn confirmation_returning_after_the_deadline(
    written: &[WindowWrite],
    late: WindowFacts,
) -> WindowFacts {
    if written.is_empty() {
        return before();
    }
    match counted_read() {
        0 => asked(),
        1 => {
            wait_until(LATE_CONFIRMATION);
            late
        }
        _ => late,
    }
}

fn late_and_moved_again(written: &[WindowWrite]) -> WindowFacts {
    confirmation_returning_after_the_deadline(written, state(60, 90, 400, 300))
}

fn late_and_unchanged(written: &[WindowWrite]) -> WindowFacts {
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

fn reacting(reaction: fn(&[WindowWrite]) -> WindowFacts) -> World {
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
fn a_read_sent_before_the_settle_budget_is_not_its_last_look() {
    move_landing(&reacting(slow_first_read_then_lands), 2_000)
        .expect("the read after the slow one is the last look, and it matched");
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
        Deadline::after(300).expect("a deadline builds"),
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
fn an_unreadable_confirmation_reports_the_read_that_matched() {
    let mut world = World::showing(before());
    world.reading = Some(unreadable_after_the_match);
    let error = move_landing(&world, 2_000).expect_err("the confirmation could not be read");
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.expect("details");
    assert_eq!(details["cause"], "readback_mismatch");
    assert_eq!(details["observed"]["x"], 300);
}
