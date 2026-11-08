use super::*;

fn maximized() -> WindowFacts {
    let mut facts = state(0, 21, 1280, 779);
    facts.maximized_vertical = true;
    facts.maximized_horizontal = true;
    facts
}

fn place() -> Extents {
    Extents {
        x: 120,
        y: 90,
        width: 500,
        height: 380,
    }
}

fn maximized_record() -> RememberedPlace {
    RememberedPlace::Maximized {
        before: place(),
        maximized: maximized().client,
    }
}

fn left_at(facts: WindowFacts) -> RememberedPlace {
    RememberedPlace::Unmaximized {
        before: place(),
        left: facts.client,
    }
}

#[test]
fn a_restore_gives_back_the_remembered_place_with_a_move_of_the_client() {
    let world = World::showing(maximized())
        .then_showing(state(121, 112, 500, 380))
        .then_showing(state(121, 112, 500, 380))
        .then_showing(state(120, 90, 500, 380))
        .keeping(maximized_record());
    run(&world, &window(), &WindowOp::Restore, budget()).expect("a restore");
    let writes = world.writes.borrow();
    assert_eq!(writes.len(), 2);
    assert_eq!(writes[0].1, WindowWrite::SetMaximized(false));
    assert_eq!(
        writes[1].1,
        WindowWrite::MoveResize {
            x: Some(120),
            y: Some(90),
            width: None,
            height: None
        }
    );
    assert_eq!(*world.remember_calls.borrow(), vec![None]);
}

#[test]
fn a_place_the_manager_did_not_give_back_is_unverified_and_stays_remembered() {
    let world = World::showing(maximized())
        .then_showing(state(121, 112, 500, 380))
        .then_showing(state(121, 112, 500, 380))
        .keeping(maximized_record());
    let error = run(&world, &window(), &WindowOp::Restore, budget()).expect_err("a miss");
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let kept = left_at(state(121, 112, 500, 380));
    assert_eq!(*world.remember_calls.borrow(), vec![Some(kept)]);
    assert_eq!(world.kept(), Some(kept));
}

fn skips_a_move_to_the_place_it_believes_current(written: &[WindowWrite]) -> WindowFacts {
    let at = |x: i32| WindowWrite::MoveResize {
        x: Some(x),
        y: Some(90),
        width: None,
        height: None,
    };
    match written.last() {
        None => maximized(),
        Some(last) if *last == at(160) => state(160, 90, 500, 380),
        Some(last) if *last == at(120) && written.contains(&at(160)) => state(120, 90, 500, 380),
        Some(_) => state(121, 112, 500, 380),
    }
}

#[test]
fn a_manager_that_skips_a_move_to_the_place_it_believes_is_current_is_nudged_away_and_back() {
    let mut world = World::showing(maximized()).keeping(maximized_record());
    world.reaction = Some(skips_a_move_to_the_place_it_believes_current);
    let deadline = Deadline::after(2_000).expect("a deadline builds");
    run(&world, &window(), &WindowOp::Restore, deadline).expect("a restore");
    let writes = world.writes.borrow();
    assert_eq!(writes.len(), 4);
    assert!(matches!(
        &writes[2].1,
        WindowWrite::MoveResize {
            x: Some(160),
            y: Some(90),
            ..
        }
    ));
}

#[test]
fn a_restore_without_time_to_confirm_keeps_the_remembered_place() {
    let facts = state(120, 90, 500, 380);
    let world = World::showing(facts).keeping(left_at(facts));
    let error = run(
        &world,
        &window(),
        &WindowOp::Restore,
        Deadline::after(100).expect("a deadline builds"),
    )
    .expect_err("the give-back needs a full confirmation pause");
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert_eq!(world.kept(), Some(left_at(facts)));
    assert!(world.remember_calls.borrow().is_empty());
}

#[test]
fn a_place_the_unmaximize_slide_only_passed_through_is_still_written_to() {
    let world = World::showing(maximized())
        .then_showing(state(80, 40, 500, 380))
        .then_showing(state(120, 90, 500, 380))
        .then_showing(state(60, 90, 500, 380))
        .then_showing(state(120, 90, 500, 380))
        .keeping(maximized_record());
    let deadline = Deadline::after(2_000).expect("a deadline builds");
    run(&world, &window(), &WindowOp::Restore, deadline).expect("the give-back lands");
    let writes = world.writes.borrow();
    assert_eq!(writes.len(), 2);
    assert_eq!(
        writes[1].1,
        WindowWrite::MoveResize {
            x: Some(120),
            y: Some(90),
            width: None,
            height: None
        }
    );
}

#[test]
fn a_retried_restore_of_an_unmaximized_window_still_gives_back_the_place_that_was_kept() {
    let world = World::showing(state(160, 90, 500, 380))
        .then_showing(state(160, 90, 500, 380))
        .then_showing(state(120, 90, 500, 380))
        .keeping(left_at(state(160, 90, 500, 380)));
    run(&world, &window(), &WindowOp::Restore, budget()).expect("the retry applies the place");
    let writes = world.writes.borrow();
    assert_eq!(writes.len(), 1);
    assert_eq!(
        writes[0].1,
        WindowWrite::MoveResize {
            x: Some(120),
            y: Some(90),
            width: None,
            height: None
        }
    );
    assert_eq!(*world.remember_calls.borrow(), vec![None]);
}

fn moved_to(x: i32, y: i32) -> WindowWrite {
    WindowWrite::MoveResize {
        x: Some(x),
        y: Some(y),
        width: None,
        height: None,
    }
}

fn now_showing(world: &World, facts: &[WindowFacts]) {
    world.facts.replace(
        facts
            .iter()
            .map(|facts| ControlRead::Ready(*facts))
            .collect(),
    );
}

fn maximized_from(before: WindowFacts) -> World {
    let world = World::showing(before).then_showing(maximized());
    run(&world, &window(), &WindowOp::Maximize, budget()).expect("a maximize");
    world
}

fn written(world: &World) -> Vec<WindowWrite> {
    world
        .writes
        .borrow()
        .iter()
        .map(|(_, write)| write.clone())
        .collect()
}

#[test]
fn a_window_unmaximized_and_moved_by_someone_else_is_not_moved_back_by_a_restore() {
    let world = maximized_from(state(120, 90, 500, 380));
    now_showing(&world, &[state(400, 300, 500, 380)]);
    let restored = run(&world, &window(), &WindowOp::Restore, budget());
    assert_eq!(written(&world), vec![WindowWrite::SetMaximized(true)]);
    restored.expect("a restore of a window that is already restored");
}

#[test]
fn a_window_unmaximized_and_moved_by_someone_else_then_minimized_is_only_lifted() {
    let world = maximized_from(state(120, 90, 500, 380));
    let mut hidden = state(400, 300, 500, 380);
    hidden.hidden = true;
    now_showing(&world, &[hidden, hidden, state(400, 300, 500, 380)]);
    run(&world, &window(), &WindowOp::Minimize, budget()).expect("a minimize");
    let restored = run(&world, &window(), &WindowOp::Restore, budget());
    assert_eq!(world.lifts.get(), 1);
    assert_eq!(
        written(&world),
        vec![WindowWrite::SetMaximized(true), WindowWrite::Iconify]
    );
    restored.expect("a lift");
}

#[test]
fn a_place_kept_for_a_retry_is_not_given_back_once_someone_else_moved_the_window() {
    let world = maximized_from(state(120, 90, 500, 380));
    now_showing(&world, &[maximized(), state(160, 90, 500, 380)]);
    run(&world, &window(), &WindowOp::Restore, budget()).expect_err("the give-back misses");
    let before = written(&world).len();
    now_showing(&world, &[state(400, 300, 500, 380)]);
    let retried = run(&world, &window(), &WindowOp::Restore, budget());
    assert_eq!(written(&world).len(), before, "{:?}", written(&world));
    retried.expect("a restore of a window that is already restored");
}

#[test]
fn a_reused_window_id_of_another_process_instance_does_not_inherit_the_place() {
    let world = maximized_from(state(120, 90, 500, 380));
    let mut reused = window();
    reused.process_instance = Some("abcd1234:100".to_owned());
    world
        .lookups
        .replace(VecDeque::from([process("abcd1234:100")]));
    now_showing(&world, &[maximized(), state(300, 200, 500, 380)]);
    run(&world, &reused, &WindowOp::Restore, budget()).expect("an un-maximize");
    assert_eq!(
        written(&world),
        vec![
            WindowWrite::SetMaximized(true),
            WindowWrite::SetMaximized(false)
        ]
    );
}

#[test]
fn a_maximize_records_the_place_before_and_the_maximized_rectangle() {
    let world = maximized_from(state(120, 90, 500, 380));
    assert_eq!(world.kept(), Some(maximized_record()));
}

#[test]
fn a_maximize_of_a_window_already_maximized_records_nothing() {
    let world = maximized_from(maximized());
    assert_eq!(world.kept(), None);
    assert!(world.remember_calls.borrow().is_empty());
}

#[test]
fn a_maximize_then_restore_gives_back_the_place_the_window_had_before() {
    let world = maximized_from(state(120, 90, 500, 380));
    now_showing(
        &world,
        &[
            maximized(),
            state(121, 112, 500, 380),
            state(121, 112, 500, 380),
            state(120, 90, 500, 380),
        ],
    );
    run(&world, &window(), &WindowOp::Restore, budget()).expect("a restore");
    assert_eq!(
        written(&world),
        vec![
            WindowWrite::SetMaximized(true),
            WindowWrite::SetMaximized(false),
            moved_to(120, 90),
        ]
    );
}
