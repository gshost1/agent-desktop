use super::*;
use serde_json::json;

#[test]
fn an_action_keeps_its_evidence_and_disposition_and_names_the_unverified_restore() {
    let result = ActionResult::delivered_unverified("notification_action")
        .with_details(json!({ "evidence": "entry_removed" }));
    let disposition = serde_json::to_value(&result).expect("json")["disposition"].clone();

    let kept = serde_json::to_value(result.with_unverified_restore()).expect("json");

    assert_eq!(kept["details"]["evidence"], "entry_removed");
    assert_eq!(kept["details"][LIST_RESTORE], "unverified");
    assert_eq!(kept["disposition"], disposition);
}

#[test]
fn a_dismissed_entry_is_returned_unchanged() {
    let info = NotificationInfo {
        index: 1,
        app_name: "Mail".into(),
        title: "a".into(),
        body: None,
        actions: Vec::new(),
    };

    let before = serde_json::to_value(&info).expect("json");

    let kept = serde_json::to_value(info.with_unverified_restore()).expect("json");

    assert_eq!(kept, before);
}

#[test]
fn details_that_are_not_an_object_are_kept_beside_the_restore() {
    let details = with_entry(Some(json!(["kept"])), LIST_RESTORE, "closed");

    assert_eq!(details["error_details"], json!(["kept"]));
    assert_eq!(details[LIST_RESTORE], "closed");
}
