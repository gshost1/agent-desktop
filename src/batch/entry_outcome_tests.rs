use serde_json::json;

use super::EntryOutcome;

fn timeout_entry(own_timeout_ms: Option<u64>) -> serde_json::Value {
    let details = own_timeout_ms.map_or_else(|| json!({}), |ms| json!({ "timeout_ms": ms }));
    json!({ "ok": false, "error": { "code": "TIMEOUT", "details": details } })
}

#[test]
fn a_timeout_whose_own_budget_outlasted_the_batch_is_the_batch_deadline() {
    let outcome = EntryOutcome::of(2, &timeout_entry(Some(5_000)), 480, true);

    assert!(outcome.failed);
    assert!(outcome.deadline_timeout);
}

#[test]
fn a_timeout_that_reports_no_budget_is_the_batch_deadline_once_it_expired() {
    assert!(EntryOutcome::of(0, &timeout_entry(None), 480, true).deadline_timeout);
}

#[test]
fn a_timeout_on_a_budget_shorter_than_the_batch_remainder_is_the_entry_own() {
    let outcome = EntryOutcome::of(1, &timeout_entry(Some(100)), 480, true);

    assert!(outcome.failed);
    assert!(!outcome.deadline_timeout);
}

#[test]
fn a_timeout_before_the_batch_deadline_expired_is_the_entry_own() {
    assert!(!EntryOutcome::of(1, &timeout_entry(Some(5_000)), 480, false).deadline_timeout);
}

#[test]
fn another_failure_or_a_success_is_never_a_deadline_timeout() {
    let stale = json!({ "ok": false, "error": { "code": "STALE_REF", "details": {} } });
    let success = json!({ "ok": true, "data": {} });

    assert!(!EntryOutcome::of(0, &stale, 0, true).deadline_timeout);
    let succeeded = EntryOutcome::of(0, &success, 0, true);
    assert!(!succeeded.failed);
    assert!(!succeeded.deadline_timeout);
}
