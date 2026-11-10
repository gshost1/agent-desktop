use super::*;

#[test]
fn a_failure_after_the_spawn_names_the_spawned_pid_and_keeps_its_details() {
    let error = with_spawned_pid(
        AdapterError::timeout("late").with_details(serde_json::json!({ "phase": "owner" })),
        4242,
    );
    let details = error.details.expect("details");
    assert_eq!(details["spawned_pid"], 4242);
    assert_eq!(details["phase"], "owner");
    let bare = with_spawned_pid(AdapterError::timeout("late"), 7);
    assert_eq!(bare.details.expect("details")["spawned_pid"], 7);
    let listed = with_spawned_pid(
        AdapterError::timeout("late").with_details(serde_json::json!([1, 2])),
        9,
    );
    let details = listed.details.expect("details");
    assert_eq!(details["spawned_pid"], 9);
    assert_eq!(details["cause"], serde_json::json!([1, 2]));
}
