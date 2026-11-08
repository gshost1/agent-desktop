use agent_desktop_core::{CommandContext, PermissionReport};
use serde_json::{Value, json};

use super::SeedPlan;
use crate::batch::{preparation::prepare, read_point::ReadPoint};

const ACTION: &str = r#"{"command": "clipboard-clear", "args": {}}"#;
const EVENT_WAIT: &str = r#"{"command": "wait", "args": {"event": "window-opened"}}"#;

fn entry(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

fn plan(commands: Value) -> (SeedPlan, usize) {
    let prepared = prepare(
        &commands.to_string(),
        &PermissionReport::default(),
        &CommandContext::default(),
    )
    .unwrap();
    (SeedPlan::new(&prepared), prepared.len())
}

fn points(commands: Value) -> Vec<Option<ReadPoint>> {
    let (plan, len) = plan(commands);
    (0..len).map(|index| plan.read_at(index)).collect()
}

fn reads(commands: Value) -> Vec<Option<usize>> {
    points(commands)
        .into_iter()
        .map(|point| point.map(ReadPoint::wait_index))
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
fn a_repeated_wait_is_read_once_after_the_earlier_wait_and_never_before_it() {
    let plan = points(json!([entry(ACTION), entry(EVENT_WAIT), entry(EVENT_WAIT)]));

    assert_eq!(
        plan,
        vec![
            Some(ReadPoint::Before { wait_index: 1 }),
            Some(ReadPoint::After { wait_index: 2 }),
            None
        ]
    );
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

#[test]
fn only_a_wait_with_the_same_whole_filter_is_read_after_the_earlier_wait() {
    let other_window =
        json!({"command": "wait", "args": {"event": "window-opened", "window": "B"}});
    let focus = json!({"command": "wait", "args": {"event": "focus-changed"}});

    let points = points(json!([
        entry(ACTION),
        entry(EVENT_WAIT),
        {"command": "version", "args": {}},
        entry(EVENT_WAIT),
        other_window,
        focus
    ]));

    assert_eq!(
        points,
        vec![
            Some(ReadPoint::Before { wait_index: 1 }),
            Some(ReadPoint::After { wait_index: 3 }),
            None,
            Some(ReadPoint::Before { wait_index: 4 }),
            Some(ReadPoint::Before { wait_index: 5 }),
            None
        ]
    );
}
