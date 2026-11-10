use agent_desktop_core::{AdapterError, Deadline, ErrorCode};

use super::{gate_observation, may_open_menu, settled_menu_observation};
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

#[test]
fn a_menu_search_past_its_node_budget_lets_the_step_act_and_a_failed_read_does_not() {
    let budget = AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Accessibility surface search exceeded its node budget",
    )
    .with_details(serde_json::json!({ "kind": "surface_search_limit", "limit": 2048 }));
    let tracking = AdapterError::new(ErrorCode::AppUnresponsive, "kAXErrorCannotComplete");
    assert_eq!(gate_observation(Err(budget)), MenuObservation::Closed);
    assert_eq!(gate_observation(Err(tracking)), MenuObservation::Unreadable);
    assert_eq!(
        gate_observation(Err(read_timeout())),
        MenuObservation::Unreadable
    );
    assert_eq!(gate_observation(Ok(true)), MenuObservation::Open);
    assert_eq!(gate_observation(Ok(false)), MenuObservation::Closed);
}
