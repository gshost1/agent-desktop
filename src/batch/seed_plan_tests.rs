use agent_desktop_core::{CommandContext, PermissionReport};
use serde_json::{Value, json};

use super::SeedPlan;
use crate::batch::preparation::prepare;

const ACTION: &str = r#"{"command": "clipboard-clear", "args": {}}"#;
const EVENT_WAIT: &str = r#"{"command": "wait", "args": {"event": "window-opened"}}"#;
const NOTIFICATION_WAIT: &str = r#"{"command": "wait", "args": {"notification": true}}"#;

fn entry(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

fn reads(commands: Value) -> Vec<Vec<usize>> {
    let prepared = prepare(
        &commands.to_string(),
        &PermissionReport::default(),
        &CommandContext::default(),
    )
    .unwrap();
    let plan = SeedPlan::new(&prepared);
    (0..prepared.len())
        .map(|index| plan.waits_read_before(index).to_vec())
        .collect()
}

#[test]
fn an_event_wait_is_read_before_the_action_across_entries_that_send_no_input() {
    let plan = reads(json!([
        entry(ACTION),
        {"command": "version", "args": {}},
        {"command": "screenshot", "args": {"output_path": "/tmp/unused.png"}},
        {"command": "wait", "args": {"ms": 10}},
        entry(EVENT_WAIT)
    ]));

    assert_eq!(plan, vec![vec![4], vec![], vec![], vec![], vec![]]);
}

#[test]
fn every_wait_after_one_action_is_read_before_that_action() {
    let plan = reads(json!([
        entry(ACTION),
        entry(NOTIFICATION_WAIT),
        entry(EVENT_WAIT),
        entry(EVENT_WAIT)
    ]));

    assert_eq!(plan, vec![vec![1, 2, 3], vec![], vec![], vec![]]);
}

#[test]
fn a_wait_is_read_before_the_nearest_action_only() {
    let plan = reads(json!([
        entry(ACTION),
        entry(EVENT_WAIT),
        entry(ACTION),
        entry(EVENT_WAIT)
    ]));

    assert_eq!(plan, vec![vec![1], vec![], vec![3], vec![]]);
}

#[test]
fn a_wait_with_no_earlier_action_and_unseeded_waits_are_never_read_for() {
    let plan = reads(json!([
        {"command": "version", "args": {}},
        entry(EVENT_WAIT),
        entry(ACTION),
        {"command": "wait", "args": {"ms": 10}}
    ]));

    assert!(plan.iter().all(Vec::is_empty), "{plan:?}");
}
