use agent_desktop_core::{CommandContext, PermissionReport};
use serde_json::{Value, json};

use super::SeedPlan;
use crate::batch::preparation::prepare;

const ACTION: &str = r#"{"command": "clipboard-clear", "args": {}}"#;
const EVENT_WAIT: &str = r#"{"command": "wait", "args": {"event": "window-opened"}}"#;

fn entry(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

fn reads(commands: Value) -> Vec<Option<usize>> {
    let prepared = prepare(
        &commands.to_string(),
        &PermissionReport::default(),
        &CommandContext::default(),
    )
    .unwrap();
    let plan = SeedPlan::new(&prepared);
    (0..prepared.len())
        .map(|index| plan.wait_read_before(index))
        .collect()
}

#[test]
fn an_event_wait_is_read_before_the_action_across_read_only_and_local_write_entries() {
    let plan = reads(json!([
        entry(ACTION),
        {"command": "version", "args": {}},
        {"command": "screenshot", "args": {"output_path": "/tmp/unused.png"}},
        {"command": "wait", "args": {"ms": 10}},
        entry(EVENT_WAIT)
    ]));

    assert_eq!(plan, vec![Some(4), None, None, None, None]);
}

#[test]
fn a_wait_after_a_wait_is_read_before_the_earlier_wait() {
    let plan = reads(json!([entry(ACTION), entry(EVENT_WAIT), entry(EVENT_WAIT)]));

    assert_eq!(plan, vec![Some(1), Some(2), None]);
}

#[test]
fn a_notification_wait_is_placed_like_an_event_wait() {
    let plan = reads(json!([
        entry(ACTION),
        {"command": "version", "args": {}},
        {"command": "wait", "args": {"notification": true}}
    ]));

    assert_eq!(plan, vec![Some(2), None, None]);
}

#[test]
fn with_no_earlier_action_the_wait_is_read_before_the_entry_right_before_it() {
    let plan = reads(json!([
        {"command": "version", "args": {}},
        {"command": "version", "args": {}},
        entry(EVENT_WAIT)
    ]));

    assert_eq!(plan, vec![None, Some(2), None]);
}

#[test]
fn a_first_entry_wait_and_unseeded_waits_are_never_read_ahead() {
    let plan = reads(json!([
        entry(EVENT_WAIT),
        entry(ACTION),
        {"command": "wait", "args": {"ms": 10}}
    ]));

    assert_eq!(plan, vec![None, None, None]);
}
