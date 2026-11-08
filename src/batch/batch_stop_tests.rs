use serde_json::json;

use super::BatchStop;

#[test]
fn a_batch_deadline_timeout_is_a_deadline_stop_whatever_stop_on_error_says() {
    for stop_on_error in [false, true] {
        let stop = BatchStop::after_entry(3, Some("TIMEOUT"), false, true, stop_on_error);

        assert_eq!(stop, Some(BatchStop::DeadlineAfterEntry { index: 3 }));
    }
}

#[test]
fn another_failure_with_stop_on_error_is_a_stop_on_error_stop() {
    let expired = BatchStop::after_entry(1, Some("STALE_REF"), false, true, true);
    let running = BatchStop::after_entry(1, Some("TIMEOUT"), false, false, true);

    assert_eq!(expired, Some(BatchStop::StopOnError { index: 1 }));
    assert_eq!(running, Some(BatchStop::StopOnError { index: 1 }));
}

#[test]
fn the_output_limit_outranks_every_other_reason() {
    let stop = BatchStop::after_entry(2, Some("TIMEOUT"), true, true, true);

    assert_eq!(stop, Some(BatchStop::OutputLimit { index: 2 }));
}

#[test]
fn a_failure_without_stop_on_error_continues_until_the_deadline() {
    assert_eq!(
        BatchStop::after_entry(0, Some("STALE_REF"), false, false, false),
        None
    );
    assert_eq!(
        BatchStop::after_entry(0, Some("STALE_REF"), false, true, false),
        Some(BatchStop::DeadlineAfterEntry { index: 0 })
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
