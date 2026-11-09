use super::{SETTLE_STEP_LABEL, is_new_raised_window, poll_within, settle_exhausted};
use crate::actions::chain::exhaustion_error;
use crate::actions::chain::menu_baseline::MenuBaseline;
use crate::actions::chain::menu_read::MenuRead;
use agent_desktop_core::{ActionStep, AdapterError, DeliverySemantics};

const SUGGESTION: &str = "Try 'mouse-click --button right --xy X,Y'.";

fn delivered_unverified_steps() -> Vec<ActionStep> {
    vec![
        ActionStep::skipped("CGClick"),
        ActionStep::succeeded("show_menu").with_verified(false),
        ActionStep::skipped("ancestor_show_menu"),
    ]
}

fn closed_baseline() -> Option<MenuBaseline> {
    Some(MenuBaseline {
        raised_windows: vec![7],
    })
}

fn scripted(
    reads: Vec<MenuRead>,
) -> (
    impl FnMut(&MenuBaseline) -> MenuRead,
    std::rc::Rc<std::cell::Cell<usize>>,
) {
    let calls = std::rc::Rc::new(std::cell::Cell::new(0));
    let counter = calls.clone();
    let mut reads = reads.into_iter();
    let observe = move |_: &MenuBaseline| {
        counter.set(counter.get() + 1);
        reads.next().unwrap_or(MenuRead::Exhausted)
    };
    (observe, calls)
}

fn assert_is_original(error: AdapterError, steps: &[ActionStep]) {
    let expected = exhaustion_error(steps, SUGGESTION);
    assert_eq!(error.code, expected.code);
    assert_eq!(error.message, expected.message);
    assert_eq!(error.suggestion, expected.suggestion);
    assert_eq!(error.disposition, expected.disposition);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert_eq!(error.details, expected.details);
}

#[test]
fn a_menu_that_appears_after_a_closed_baseline_verifies_the_delivery() {
    let (observe, calls) = scripted(vec![MenuRead::Appeared]);

    let steps = settle_exhausted(
        delivered_unverified_steps(),
        closed_baseline(),
        observe,
        SUGGESTION,
    )
    .expect("an observed menu settles the chain");

    assert_eq!(calls.get(), 1);
    let result = agent_desktop_core::ActionResult::from_execution(
        &agent_desktop_core::Action::RightClick,
        steps.clone(),
    );
    assert_eq!(
        result.disposition(),
        DeliverySemantics::delivered_verified()
    );
    assert_eq!(steps.last().map(ActionStep::label), Some(SETTLE_STEP_LABEL));
    assert_eq!(steps[1].verified(), None);
}

#[test]
fn no_menu_keeps_the_original_error_unchanged() {
    let (observe, calls) = scripted(vec![MenuRead::Absent, MenuRead::Absent]);

    let error = settle_exhausted(
        delivered_unverified_steps(),
        closed_baseline(),
        observe,
        SUGGESTION,
    )
    .expect_err("no menu must not upgrade");

    assert_eq!(calls.get(), 3);
    assert_is_original(error, &delivered_unverified_steps());
}

#[test]
fn without_a_closed_baseline_a_menu_is_not_an_upgrade() {
    let (observe, calls) = scripted(vec![MenuRead::Appeared]);

    let error = settle_exhausted(delivered_unverified_steps(), None, observe, SUGGESTION)
        .expect_err("an unproven baseline must not upgrade");

    assert_eq!(calls.get(), 0);
    assert_is_original(error, &delivered_unverified_steps());
}

#[test]
fn an_exhausted_read_budget_keeps_the_original_error() {
    let deadline = agent_desktop_core::Deadline::after(0).expect("zero-duration deadline is valid");
    let (read, calls) = scripted(vec![MenuRead::Appeared]);

    let error = settle_exhausted(
        delivered_unverified_steps(),
        closed_baseline(),
        poll_within(deadline, read),
        SUGGESTION,
    )
    .expect_err("a spent operation deadline must not read or upgrade");

    assert_eq!(calls.get(), 0);
    assert_is_original(error, &delivered_unverified_steps());
}

#[test]
fn a_chain_that_delivered_nothing_is_never_settled() {
    let steps = vec![ActionStep::skipped("show_menu")];
    let (observe, calls) = scripted(vec![MenuRead::Appeared]);

    let error = settle_exhausted(steps.clone(), closed_baseline(), observe, SUGGESTION)
        .expect_err("a menu the chain did not cause is not its effect");

    assert_eq!(calls.get(), 0);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(error.message, exhaustion_error(&steps, SUGGESTION).message);
}

#[test]
fn only_a_raised_window_absent_from_the_baseline_is_new() {
    let baseline = MenuBaseline {
        raised_windows: vec![7, 9],
    };

    assert!(!is_new_raised_window(&baseline, &[]));
    assert!(!is_new_raised_window(&baseline, &[9, 7]));
    assert!(is_new_raised_window(&baseline, &[7, 12]));
}
