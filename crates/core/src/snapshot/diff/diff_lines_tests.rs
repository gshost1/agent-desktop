use super::*;
use serde_json::json;

#[test]
fn rows_become_marked_lines_and_the_status_stays() {
    let mut data = json!({
        "snapshot_id": "s2",
        "diff": {
            "since": "s1",
            "status": "changed",
            "added": [{ "ref_id": "@s2:e3", "role": "button", "name": "Close" }],
            "changed": [{
                "ref_id": "@s2:e1", "role": "textfield", "name": "Name", "value": "abc",
                "states": ["focused"],
                "changed": ["value", "focus"],
                "was": { "value": "ab", "states": [] }
            }],
            "removed": [{ "role": "statictext", "name": "Loading" }],
            "focused": { "ref_id": "@s2:e1", "role": "textfield", "name": "Name", "value": "abc", "states": ["focused"] }
        }
    });
    assert!(replace_diff_with_lines(&mut data).unwrap());
    assert_eq!(
        data["lines"],
        json!([
            "+ @s2:e3 button \"Close\"",
            "~ @s2:e1 textfield \"Name\" value=\"abc\" [focused] (was value=\"ab\", [])",
            "- statictext \"Loading\"",
            "focused @s2:e1 textfield \"Name\" value=\"abc\" [focused]"
        ])
    );
    assert_eq!(data["diff"], json!({ "since": "s1", "status": "changed" }));
}

#[test]
fn no_change_is_stated_in_the_lines() {
    let mut data = json!({ "diff": { "since": "s1", "status": "unchanged" } });
    assert!(replace_diff_with_lines(&mut data).unwrap());
    assert_eq!(data["lines"], json!(["no change"]));
}

#[test]
fn an_earlier_absent_field_reads_as_none() {
    let mut data = json!({ "diff": { "since": "s1", "status": "changed", "changed": [{
        "role": "textfield", "name": "Name", "value": "a", "description": "Help",
        "changed": ["value", "description"], "was": { "value": null, "description": "Old" }
    }]}});
    assert!(replace_diff_with_lines(&mut data).unwrap());
    assert_eq!(
        data["lines"][0],
        "~ textfield \"Name\" value=\"a\" desc=\"Help\" (was no value, desc=\"Old\")"
    );
}

#[test]
fn a_not_comparable_diff_or_no_diff_is_left_alone() {
    let mut data = json!({ "tree": { "role": "window" }, "diff": { "since": "s1", "status": "not_comparable", "reason": "owner_changed" } });
    let before = data.clone();
    assert!(!replace_diff_with_lines(&mut data).unwrap());
    assert_eq!(data, before);
    let mut plain = json!({ "snapshot_id": "s2" });
    assert!(!replace_diff_with_lines(&mut plain).unwrap());
}

#[test]
fn a_row_that_does_not_read_back_changes_nothing() {
    let mut data = json!({ "diff": { "since": "s1", "status": "changed", "added": [{ "name": "no role" }] } });
    let before = data.clone();
    assert!(replace_diff_with_lines(&mut data).is_err());
    assert_eq!(data, before);
}
