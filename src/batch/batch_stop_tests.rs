use serde_json::json;

use super::BatchStop;
use crate::batch::entry_outcome::EntryOutcome;

fn outcome(failed: bool, deadline_timeout: bool, oversized: bool) -> EntryOutcome {
    EntryOutcome {
        index: 3,
        failed,
        deadline_timeout,
        oversized,
    }
}

#[test]
fn a_batch_deadline_timeout_is_a_deadline_stop_on_any_entry_whatever_stop_on_error_says() {
    for (deadline_stops_rest, stop_on_error) in
        [(true, false), (true, true), (false, false), (false, true)]
    {
        let stop = BatchStop::after_entry(
            &outcome(true, true, false),
            deadline_stops_rest,
            stop_on_error,
        );

        assert_eq!(stop, Some(BatchStop::DeadlineAfterEntry { index: 3 }));
    }
}

#[test]
fn another_failure_with_stop_on_error_is_a_stop_on_error_stop() {
    for deadline_stops_rest in [false, true] {
        let stop = BatchStop::after_entry(&outcome(true, false, false), deadline_stops_rest, true);

        assert_eq!(stop, Some(BatchStop::StopOnError { index: 3 }));
    }
}

#[test]
fn the_output_limit_outranks_every_other_reason() {
    let stop = BatchStop::after_entry(&outcome(true, true, true), true, true);

    assert_eq!(stop, Some(BatchStop::OutputLimit { index: 3 }));
}

#[test]
fn a_failure_without_stop_on_error_continues_until_the_deadline() {
    assert_eq!(
        BatchStop::after_entry(&outcome(true, false, false), false, false),
        None
    );
    assert_eq!(
        BatchStop::after_entry(&outcome(true, false, false), true, false),
        Some(BatchStop::DeadlineAfterEntry { index: 3 })
    );
}

#[test]
fn each_stop_keeps_its_published_shape() {
    assert_eq!(
        BatchStop::DeadlineBeforeEntry { index: 1 }.to_value(),
        json!({"reason": "deadline", "index": 1})
    );
    assert_eq!(
        BatchStop::DeadlineAfterEntry { index: 1 }.to_value(),
        json!({"reason": "deadline", "after_index": 1})
    );
    assert_eq!(
        BatchStop::BaselineFailed {
            blocked_index: 0,
            blocked_command: "click".into(),
            wait_index: 2,
            wait_command: "wait".into(),
        }
        .to_value(),
        json!({
            "reason": "pre_action_baseline_failed",
            "blocked_index": 0,
            "blocked_command": "click",
            "wait_index": 2,
            "wait_command": "wait",
        })
    );
}
