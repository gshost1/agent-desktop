use super::{
    SETTLE_STEP_LABEL, baseline_from, is_new_menu_window, poll_within, read_from, settle_exhausted,
};
use crate::actions::chain::exhaustion_error;
use crate::actions::chain::menu_baseline::MenuBaseline;
use crate::actions::chain::menu_read::MenuRead;
use agent_desktop_core::{ActionStep, AdapterError, DeliverySemantics};

const SETTLE: std::time::Duration = std::time::Duration::from_millis(600);
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
        menu_windows: vec![7],
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
    let (mut read, calls) = scripted(vec![MenuRead::Appeared]);

    let error = settle_exhausted(
        delivered_unverified_steps(),
        closed_baseline(),
        poll_within(deadline, SETTLE, move |baseline, _| read(baseline)),
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
fn only_a_menu_window_absent_from_the_baseline_is_new() {
    let baseline = MenuBaseline {
        menu_windows: vec![7, 9],
    };

    assert!(!is_new_menu_window(&baseline, &[]));
    assert!(!is_new_menu_window(&baseline, &[9, 7]));
    assert!(is_new_menu_window(&baseline, &[7, 12]));
}

fn unreadable() -> AdapterError {
    AdapterError::new(agent_desktop_core::ErrorCode::AppUnresponsive, "no answer")
}

#[test]
fn a_menu_already_open_at_baseline_leaves_no_baseline() {
    let mut window_reads = 0;

    let baseline = baseline_from(Ok(true), || {
        window_reads += 1;
        Ok(Vec::new())
    });

    assert_eq!(baseline, None);
    assert_eq!(window_reads, 0);
    assert_eq!(baseline_from(Err(unreadable()), || Ok(Vec::new())), None);
    assert_eq!(baseline_from(Ok(false), || Err(unreadable())), None);
    assert_eq!(
        baseline_from(Ok(false), || Ok(vec![3])),
        Some(MenuBaseline {
            menu_windows: vec![3]
        })
    );
}

#[test]
fn a_new_menu_window_counts_without_the_accessibility_read() {
    let baseline = closed_baseline().unwrap_or_default();
    let mut ax_reads = 0;

    let read = read_from(&baseline, Ok(vec![7, 12]), || {
        ax_reads += 1;
        Ok(false)
    });

    assert_eq!(read, MenuRead::Appeared);
    assert_eq!(ax_reads, 0);
}

#[test]
fn an_accessibility_menu_counts_when_the_window_read_sees_none() {
    let baseline = closed_baseline().unwrap_or_default();

    assert_eq!(
        read_from(&baseline, Ok(vec![7]), || Ok(true)),
        MenuRead::Appeared
    );
    assert_eq!(
        read_from(&baseline, Err(unreadable()), || Ok(true)),
        MenuRead::Appeared
    );
    assert_eq!(
        read_from(&baseline, Ok(vec![7]), || Ok(false)),
        MenuRead::Absent
    );
    assert_eq!(
        read_from(&baseline, Err(unreadable()), || Err(unreadable())),
        MenuRead::Absent
    );
}

#[test]
fn polling_stops_at_the_settle_cutoff_with_a_live_deadline() {
    let deadline = agent_desktop_core::Deadline::after(3_000).expect("deadline is valid");
    let settle = std::time::Duration::from_millis(80);
    let started = std::time::Instant::now();
    let mut reads = 0;

    let mut poll = poll_within(deadline, settle, |_, _| {
        reads += 1;
        MenuRead::Absent
    });
    let baseline = MenuBaseline::default();
    let end = loop {
        let read = poll(&baseline);
        if read != MenuRead::Absent {
            break read;
        }
    };

    assert_eq!(end, MenuRead::Exhausted);
    assert!(started.elapsed() < std::time::Duration::from_millis(1_000));
    assert!(!deadline.is_expired());
    drop(poll);
    assert!(reads >= 1);
}

#[test]
fn every_read_is_capped_to_the_settle_window() {
    let deadline = agent_desktop_core::Deadline::after(3_000).expect("deadline is valid");
    let settle = std::time::Duration::from_millis(80);
    let mut longest = std::time::Duration::ZERO;

    let mut poll = poll_within(deadline, settle, |_, read_deadline| {
        longest = longest.max(read_deadline.remaining());
        MenuRead::Absent
    });
    let baseline = MenuBaseline::default();
    while poll(&baseline) == MenuRead::Absent {}
    drop(poll);

    assert!(longest > std::time::Duration::ZERO);
    assert!(longest <= settle);
}
