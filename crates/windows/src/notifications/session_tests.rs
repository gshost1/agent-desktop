use agent_desktop_core::{
    ActionResult, AdapterError, ErrorCode, InteractionPolicy, NotificationInfo,
};
use serde_json::json;

use super::{closed_center_policy_error, merge_session_result};

fn entry(title: &str) -> NotificationInfo {
    NotificationInfo {
        index: 1,
        app_name: "Mail".into(),
        title: title.into(),
        body: None,
        actions: Vec::new(),
    }
}

fn cleanup_failure() -> Result<(), AdapterError> {
    Err(AdapterError::timeout("cleanup failed"))
}

#[test]
fn a_closed_center_is_policy_denied_headlessly() {
    let error = closed_center_policy_error(InteractionPolicy::headless());

    assert_eq!(error.code, ErrorCode::PolicyDenied);
    assert!(
        error
            .suggestion
            .as_deref()
            .is_some_and(|value| value.contains("--headed"))
    );
}

#[test]
fn the_operation_error_wins_when_cleanup_also_fails_and_names_the_restore() {
    let operation = AdapterError::new(ErrorCode::ElementNotFound, "operation failed");

    let error =
        merge_session_result::<NotificationInfo>(Err(operation), cleanup_failure()).unwrap_err();

    assert_eq!(error.code, ErrorCode::ElementNotFound);
    assert_eq!(
        error.details.expect("details")["list_restore"],
        "unverified"
    );
}

#[test]
fn a_delivered_action_keeps_its_result_when_cleanup_fails() {
    let pressed = ActionResult::delivered_unverified("notification_action")
        .with_details(json!({ "evidence": "entry_removed" }));
    let disposition = serde_json::to_value(&pressed).expect("json")["disposition"].clone();

    let kept = merge_session_result(Ok(pressed), cleanup_failure())
        .expect("the delivered action stays the answer");

    let kept = serde_json::to_value(kept).expect("json");
    assert_eq!(kept["details"]["list_restore"], "unverified");
    assert_eq!(kept["details"]["evidence"], "entry_removed");
    assert_eq!(kept["disposition"], disposition);
}

#[test]
fn a_delivered_dismissal_keeps_the_dismissed_entry_when_cleanup_fails() {
    let kept = merge_session_result(Ok(entry("a")), cleanup_failure())
        .expect("the delivered dismissal stays the answer");

    assert_eq!(kept.title, "a");
}

#[test]
fn a_delivered_clear_keeps_its_counts_when_cleanup_fails() {
    let cleared = (vec![entry("a"), entry("b")], vec!["c".to_owned()]);

    let (dismissed, failures) = merge_session_result(Ok(cleared), cleanup_failure())
        .expect("the delivered clear stays the answer");

    assert_eq!(dismissed.len(), 2);
    assert_eq!(failures, vec!["c".to_owned()]);
}

#[test]
fn a_read_whose_cleanup_fails_reports_the_cleanup_failure() {
    let error = merge_session_result(Ok(vec![entry("a")]), cleanup_failure()).unwrap_err();

    assert_eq!(error.code, ErrorCode::Timeout);
}

#[test]
fn a_clean_cleanup_passes_the_operation_through() {
    let value = merge_session_result(Ok(entry("a")), Ok(())).expect("the value survives");

    assert_eq!(value.title, "a");
}
