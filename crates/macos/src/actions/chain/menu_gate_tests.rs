use agent_desktop_core::{AdapterError, Deadline, ErrorCode};

use super::{gate_answer, may_open_menu, settled_menu_observation};
use crate::actions::chain::menu_observation::MenuObservation;

fn read_timeout() -> AdapterError {
    AdapterError::timeout("menu read did not finish")
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

fn budget_exhausted() -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Accessibility surface search exceeded its node budget",
    )
    .with_details(serde_json::json!({ "kind": "surface_search_limit", "limit": 2048 }))
}

fn not_read() -> Result<bool, AdapterError> {
    panic!("a menu window on screen answers without the accessibility read")
}

#[test]
fn a_dense_window_with_a_menu_window_on_screen_refuses_a_second_menu() {
    assert_eq!(gate_answer(Ok(vec![42]), not_read), MenuObservation::Open);
    assert_eq!(
        gate_answer(Ok(vec![42]), || Err(budget_exhausted())),
        MenuObservation::Open
    );
}

#[test]
fn a_dense_window_with_no_menu_window_lets_the_step_act() {
    assert_eq!(
        gate_answer(Ok(Vec::new()), || Err(budget_exhausted())),
        MenuObservation::Closed
    );
}

#[test]
fn a_capped_search_without_a_window_server_answer_refuses() {
    let window_server_failed = Err(AdapterError::internal("window list unavailable"));
    assert_eq!(
        gate_answer(window_server_failed, || Err(budget_exhausted())),
        MenuObservation::Unreadable
    );
}

#[test]
fn any_other_failed_accessibility_read_refuses_and_a_clean_read_decides() {
    let tracking = || {
        Err(AdapterError::new(
            ErrorCode::AppUnresponsive,
            "kAXErrorCannotComplete",
        ))
    };
    assert_eq!(
        gate_answer(Ok(Vec::new()), tracking),
        MenuObservation::Unreadable
    );
    assert_eq!(
        gate_answer(Ok(Vec::new()), || Err(read_timeout())),
        MenuObservation::Unreadable
    );
    assert_eq!(
        gate_answer(Ok(Vec::new()), || Ok(true)),
        MenuObservation::Open
    );
    assert_eq!(
        gate_answer(Ok(Vec::new()), || Ok(false)),
        MenuObservation::Closed
    );
}
