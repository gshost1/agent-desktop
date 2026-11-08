use serde_json::json;

use super::EntryOutcome;

fn timeout_entry() -> serde_json::Value {
    json!({ "ok": false, "error": { "code": "TIMEOUT", "details": { "timeout_ms": 100 } } })
}

#[test]
fn a_timeout_after_the_batch_capped_deadline_expired_is_the_batch_deadline() {
    let outcome = EntryOutcome::of(2, &timeout_entry(), true);

    assert!(outcome.failed);
    assert!(outcome.deadline_timeout);
}

#[test]
fn a_timeout_without_that_record_is_the_entry_own() {
    let outcome = EntryOutcome::of(1, &timeout_entry(), false);

    assert!(outcome.failed);
    assert!(!outcome.deadline_timeout);
}

#[test]
fn another_failure_or_a_success_is_never_a_deadline_timeout() {
    let stale = json!({ "ok": false, "error": { "code": "STALE_REF", "details": {} } });
    let success = json!({ "ok": true, "data": {} });

    assert!(!EntryOutcome::of(0, &stale, true).deadline_timeout);
    let succeeded = EntryOutcome::of(0, &success, true);
    assert!(!succeeded.failed);
    assert!(!succeeded.deadline_timeout);
}
