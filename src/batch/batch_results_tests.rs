use serde_json::json;

use super::BatchResults;

#[test]
fn a_completed_entry_that_returned_an_error_counts_as_failed() {
    let mut results = BatchResults::with_capacity(3);
    results.push(json!({ "index": 0, "execution": "completed", "ok": true }));
    results.push(json!({ "index": 1, "execution": "completed", "ok": false }));
    results.push(json!({ "index": 2, "execution": "not_started", "ok": false }));

    assert_eq!(results.completed(), 2);
    assert_eq!(results.failed(), 1);
}
