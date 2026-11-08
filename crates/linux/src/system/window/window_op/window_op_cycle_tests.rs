use super::*;

fn hidden(mut facts: WindowFacts) -> WindowFacts {
    facts.hidden = true;
    facts
}

fn give_back_reads() -> [WindowFacts; 3] {
    [
        state(121, 112, 500, 380),
        state(121, 112, 500, 380),
        state(120, 90, 500, 380),
    ]
}

fn ends_with_the_place(world: &World) {
    let writes = written(world);
    assert_eq!(writes.last(), Some(&moved_to(120, 90)), "{writes:?}");
}

#[test]
fn a_maximize_the_manager_applies_after_the_deadline_still_gets_its_place_back() {
    let world = World::showing(state(120, 90, 500, 380));
    run(&world, &window(), &WindowOp::Maximize, budget()).expect_err("not seen in time");
    let [first, second, last] = give_back_reads();
    now_showing(&world, &[maximized(), first, second, last]);
    run(&world, &window(), &WindowOp::Restore, budget()).expect("a restore");
    ends_with_the_place(&world);
}

#[test]
fn an_earlier_cycle_record_does_not_survive_a_new_maximize() {
    let world = World::showing(state(120, 90, 500, 380)).keeping(RememberedPlace::Maximized {
        before: Extents {
            x: 300,
            y: 200,
            width: 500,
            height: 380,
        },
        maximized: maximized().client,
    });
    run(&world, &window(), &WindowOp::Maximize, budget()).expect_err("not seen in time");
    let [first, second, last] = give_back_reads();
    now_showing(&world, &[maximized(), first, second, last]);
    run(&world, &window(), &WindowOp::Restore, budget()).expect("a restore");
    ends_with_the_place(&world);
    assert!(!written(&world).contains(&moved_to(300, 200)));
}

#[test]
fn a_maximized_rectangle_seen_mid_animation_is_not_kept_as_the_maximized_place() {
    let mut frame = state(40, 60, 1200, 700);
    frame.maximized_vertical = true;
    frame.maximized_horizontal = true;
    let world = World::showing(state(120, 90, 500, 380))
        .then_showing(frame)
        .then_showing(maximized());
    run(&world, &window(), &WindowOp::Maximize, budget()).expect("a maximize");
    let kept = world.kept();
    assert!(
        !matches!(kept, Some(RememberedPlace::Maximized { maximized, .. }) if maximized == frame.client),
        "{kept:?}"
    );
}

#[test]
fn a_restore_whose_read_after_the_lift_is_still_hidden_gives_the_place_back() {
    let [first, second, last] = give_back_reads();
    let world = World::showing(hidden(maximized()))
        .then_showing(hidden(maximized()))
        .then_showing(first)
        .then_showing(second)
        .then_showing(last)
        .keeping(maximized_record());
    run(&world, &window(), &WindowOp::Restore, budget()).expect("a restore");
    assert_eq!(world.lifts.get(), 1);
    ends_with_the_place(&world);
}

#[test]
fn a_hidden_read_that_matches_no_record_keeps_the_record() {
    let world = World::showing(hidden(state(400, 300, 500, 380))).keeping(maximized_record());
    run(&world, &window(), &WindowOp::Restore, budget()).expect("a lift");
    assert_eq!(world.kept(), Some(maximized_record()));
    assert!(written(&world).is_empty());
}

#[test]
fn a_new_maximize_from_where_a_restore_left_the_window_carries_the_first_place_forward() {
    let left = state(160, 90, 500, 380);
    let world = World::showing(left)
        .then_showing(maximized())
        .keeping(left_at(left));
    run(&world, &window(), &WindowOp::Maximize, budget()).expect("a maximize");
    let kept = world.kept();
    assert!(
        matches!(kept, Some(RememberedPlace::Maximized { before, .. }) if before == place()),
        "{kept:?}"
    );
}
