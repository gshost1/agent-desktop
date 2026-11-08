use std::time::Instant;

use super::*;
use crate::system::window::window_op::window_op_verdict::{POLL, SETTLE_BUDGET};

#[test]
fn a_resize_with_continuous_rectangle_drift_is_not_verified() {
    fn drifting(_: &[WindowWrite]) -> WindowFacts {
        thread_local! {
            static POSITION: Cell<bool> = const { Cell::new(false) };
        }
        let x = POSITION.with(|position| i32::from(position.replace(!position.get())) * 20);
        state(x, 0, 520, 380)
    }
    let mut world = World::showing(state(0, 0, 400, 300));
    world.reaction = Some(drifting);
    let (verdict, _) = super::super::settle(
        &world,
        CLIENT,
        &requested(&resize(520.0, 380.0)).unwrap().unwrap(),
        Deadline::after(500).unwrap(),
    );
    assert_eq!(verdict, Verdict::Mismatch);
}

#[test]
fn unreadable_ownership_after_a_write_is_unverified_not_missing() {
    let mut unreadable_client = client(CLIENT, "Doc");
    unreadable_client.pid = None;
    for (lookup, clients) in [
        (
            Lookup::Unreadable,
            ControlRead::Ready(vec![client(CLIENT, "Doc")]),
        ),
        (process(INSTANCE), ControlRead::Unavailable),
        (
            process(INSTANCE),
            ControlRead::Ready(vec![unreadable_client]),
        ),
    ] {
        let mut maximized = state(0, 0, 400, 300);
        maximized.maximized_vertical = true;
        maximized.maximized_horizontal = true;
        let world = World::showing(maximized);
        world.lookups.replace([process(INSTANCE), lookup].into());
        world
            .clients
            .replace([ControlRead::Ready(vec![client(CLIENT, "Doc")]), clients].into());
        let error = run(&world, &window(), &WindowOp::Maximize, budget())
            .expect_err("ownership cannot be verified");
        assert_eq!(error.code, ErrorCode::ActionFailed);
        assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
        assert_eq!(
            error.details.expect("cause")["cause"],
            "ownership_unverified"
        );
        assert_eq!(world.writes(), 1);
    }
}

#[test]
fn a_move_seen_only_in_passing_is_not_the_held_result() {
    let world = World::showing(state(10, 10, 400, 300))
        .then_showing(state(300, 200, 400, 300))
        .then_showing(state(60, 90, 400, 300));
    let error = run(
        &world,
        &window(),
        &WindowOp::Move { x: 300.0, y: 200.0 },
        budget(),
    )
    .expect_err("the slide kept going past the asked place");
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.expect("details");
    assert_eq!(details["observed"]["x"], 60);
    assert_eq!(details["observed"]["y"], 90);
}

#[test]
fn a_move_without_time_for_the_confirmation_pause_is_unverified() {
    let world = World::showing(state(300, 200, 400, 300));
    let error = run(
        &world,
        &window(),
        &WindowOp::Move { x: 300.0, y: 200.0 },
        Deadline::after(100).expect("a deadline builds"),
    )
    .expect_err("two immediate reads cannot prove the place held");
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert_eq!(world.writes(), 1);
}

fn confirmation_returning_after_the_limit(
    written: &[WindowWrite],
    late: WindowFacts,
) -> WindowFacts {
    thread_local! {
        static READS: Cell<u32> = const { Cell::new(0) };
    }
    if written.is_empty() {
        return state(10, 10, 400, 300);
    }
    let first = READS.with(|reads| reads.replace(reads.get() + 1) == 0);
    if first {
        state(300, 200, 400, 300)
    } else {
        std::thread::sleep(SETTLE_BUDGET);
        late
    }
}

fn late_and_moved_again(written: &[WindowWrite]) -> WindowFacts {
    confirmation_returning_after_the_limit(written, state(60, 90, 400, 300))
}

fn late_and_unchanged(written: &[WindowWrite]) -> WindowFacts {
    confirmation_returning_after_the_limit(written, state(300, 200, 400, 300))
}

fn move_confirmed_late(reaction: fn(&[WindowWrite]) -> WindowFacts) -> AdapterError {
    let mut world = World::showing(state(10, 10, 400, 300));
    world.reaction = Some(reaction);
    run(
        &world,
        &window(),
        &WindowOp::Move { x: 300.0, y: 200.0 },
        Deadline::after(2_000).expect("a deadline builds"),
    )
    .expect_err("a confirmation read past the limit cannot prove the place held")
}

#[test]
fn a_confirmation_that_returns_after_the_limit_reports_its_latest_rectangle() {
    let error = move_confirmed_late(late_and_moved_again);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.expect("details");
    assert_eq!(details["observed"]["x"], 60);
    assert_eq!(details["observed"]["y"], 90);
}

#[test]
fn a_confirmation_that_agrees_but_returns_after_the_limit_is_not_a_held_place() {
    let error = move_confirmed_late(late_and_unchanged);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}

#[test]
fn a_replacement_client_cannot_confirm_the_original_windows_operation() {
    let mut maximized = state(0, 0, 400, 300);
    maximized.maximized_vertical = true;
    maximized.maximized_horizontal = true;
    let world = World::showing(maximized);
    let mut replacement = client(CLIENT, "Doc");
    replacement.pid = Some(999);
    world.clients.replace(
        [
            ControlRead::Ready(vec![client(CLIENT, "Doc")]),
            ControlRead::Ready(vec![replacement]),
        ]
        .into(),
    );
    let error = run(&world, &window(), &WindowOp::Maximize, budget())
        .expect_err("a replacement's matching state is not delivery proof");
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let current = World::showing(maximized);
    run(&current, &window(), &WindowOp::Maximize, budget()).expect("the current owner confirms");
}

fn landing_after(written: &[WindowWrite], after: Duration) -> WindowFacts {
    thread_local! {
        static WRITTEN_AT: Cell<Option<Instant>> = const { Cell::new(None) };
    }
    if written.is_empty() {
        return state(10, 10, 400, 300);
    }
    let since = WRITTEN_AT.with(|at| *at.get().get_or_insert_with(Instant::now));
    WRITTEN_AT.with(|at| at.set(Some(since)));
    if since.elapsed() >= after {
        state(300, 200, 400, 300)
    } else {
        state(10, 10, 400, 300)
    }
}

fn lands_after_most_of_the_settle_budget(written: &[WindowWrite]) -> WindowFacts {
    landing_after(written, Duration::from_millis(150))
}

fn lands_after_the_settle_budget(written: &[WindowWrite]) -> WindowFacts {
    landing_after(written, SETTLE_BUDGET + Duration::from_millis(50))
}

fn move_landing(
    reaction: fn(&[WindowWrite]) -> WindowFacts,
    deadline_ms: u64,
) -> (World, Result<OperationReport, AdapterError>) {
    let mut world = World::showing(state(10, 10, 400, 300));
    world.reaction = Some(reaction);
    let outcome = run(
        &world,
        &window(),
        &WindowOp::Move { x: 300.0, y: 200.0 },
        Deadline::after(deadline_ms).expect("a deadline builds"),
    );
    (world, outcome)
}

#[test]
fn a_place_the_manager_applies_late_in_the_settle_budget_still_gets_its_confirmation() {
    let (world, outcome) = move_landing(lands_after_most_of_the_settle_budget, 2_000);
    outcome.expect("the place matched within the settle budget and held through the pause");
    assert_eq!(world.writes(), 1);
}

#[test]
fn a_place_first_seen_after_the_settle_budget_is_unverified() {
    let (_, outcome) = move_landing(lands_after_the_settle_budget, 2_000);
    let error = outcome.expect_err("no read matched before the settle budget ran out");
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}

fn matching_read_that_returns_after_the_settle_budget(written: &[WindowWrite]) -> WindowFacts {
    thread_local! {
        static WRITTEN_AT: Cell<Option<Instant>> = const { Cell::new(None) };
    }
    if written.is_empty() {
        return state(10, 10, 400, 300);
    }
    let since = WRITTEN_AT.with(|at| *at.get().get_or_insert_with(Instant::now));
    WRITTEN_AT.with(|at| at.set(Some(since)));
    if since.elapsed() < SETTLE_BUDGET - POLL {
        return state(10, 10, 400, 300);
    }
    let past = SETTLE_BUDGET + POLL;
    std::thread::sleep(past.saturating_sub(since.elapsed()));
    state(300, 200, 400, 300)
}

#[test]
fn a_match_read_back_only_after_the_settle_budget_gets_no_confirmation() {
    let (_, outcome) = move_landing(matching_read_that_returns_after_the_settle_budget, 2_000);
    let error = outcome.expect_err("the read that matched came back after the settle budget");
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}

#[test]
fn a_late_match_gets_no_confirmation_past_the_command_deadline() {
    let mut world = World::showing(state(10, 10, 400, 300));
    world.reaction = Some(lands_after_most_of_the_settle_budget);
    world
        .write(CLIENT, &super::super::hold::move_to(300, 200))
        .expect("the fake write lands");
    let (verdict, observed) = super::super::settle(
        &world,
        CLIENT,
        &super::super::hold::move_to(300, 200),
        Deadline::after(200).expect("a deadline builds"),
    );
    assert_eq!(verdict, Verdict::Mismatch);
    assert_eq!(observed.map(|facts| facts.client.x), Some(300));
}
