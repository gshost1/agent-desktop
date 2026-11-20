use agent_desktop_core::{DeliverySemantics, ErrorCode, NotificationInfo};

use super::{
    action_changed_state, departed, dismiss_survived_error, entry_gone, matching_count,
    same_identity, survivor_failures,
};

fn info(index: usize, app: &str, title: &str, body: Option<&str>) -> NotificationInfo {
    NotificationInfo {
        index,
        app_name: app.into(),
        title: title.into(),
        body: body.map(String::from),
        actions: Vec::new(),
    }
}

#[test]
fn identity_includes_app_title_and_body() {
    let original = info(1, "App A", "Reminder", Some("First"));

    assert!(same_identity(
        &original,
        &info(2, "App A", "Reminder", Some("First"))
    ));
    assert!(!same_identity(
        &original,
        &info(1, "App B", "Reminder", Some("First"))
    ));
    assert!(!same_identity(
        &original,
        &info(1, "App A", "Changed", Some("First"))
    ));
    assert!(!same_identity(
        &original,
        &info(1, "App A", "Reminder", Some("Second"))
    ));
}

#[test]
fn matching_count_counts_identity_matches_not_indices() {
    let target = info(1, "App A", "Reminder", Some("First"));
    let current = vec![
        info(1, "App A", "Reminder", Some("First")),
        info(2, "App B", "Other", None),
        info(3, "App A", "Reminder", Some("First")),
    ];

    assert_eq!(matching_count(&current, &target), 2);
}

#[test]
fn an_entry_is_gone_only_when_no_identity_match_remains() {
    let target = info(1, "App A", "Reminder", Some("First"));

    assert!(entry_gone(&target, &[info(1, "App B", "Other", None)]));
    assert!(!entry_gone(
        &target,
        &[info(3, "App A", "Reminder", Some("First"))]
    ));
    assert!(entry_gone(&target, &[]));
}

#[test]
fn an_ignored_clear_reports_every_captured_member_as_a_failure() {
    let captured = vec![
        info(1, "App A", "first", None),
        info(2, "App B", "second", None),
    ];
    let after = vec![
        info(1, "App A", "first", None),
        info(2, "App B", "second", None),
    ];

    let failures = survivor_failures(&captured, &after);

    assert_eq!(
        failures.len(),
        2,
        "an ignored clear must report each captured member, which is exactly the case an emptiness check cannot tell from a full clear"
    );
    assert!(
        failures
            .iter()
            .all(|failure| failure.contains("#1") || failure.contains("#2"))
    );
}

#[test]
fn a_repost_during_the_clear_reports_zero_failures() {
    let captured = vec![
        info(1, "App A", "first", None),
        info(2, "App B", "second", None),
    ];
    let reposted = vec![info(1, "App A", "first", Some("new body arrives later"))];

    let failures = survivor_failures(&captured, &reposted);

    assert!(
        failures.is_empty(),
        "the new entry is an arrival outside the captured set, not a survivor"
    );
}

#[test]
fn a_partial_clear_reports_only_the_captured_survivors() {
    let captured = vec![
        info(1, "App A", "cleared", None),
        info(2, "App B", "survivor", None),
    ];
    let after = vec![
        info(1, "App B", "survivor", None),
        info(2, "App C", "new arrival", None),
    ];

    let failures = survivor_failures(&captured, &after);

    assert_eq!(failures.len(), 1);
    assert!(failures[0].contains("#2"));
}

#[test]
fn failure_strings_name_the_index_and_never_the_content() {
    let captured = vec![info(4, "Secret App", "Secret Title", Some("Secret Body"))];
    let after = vec![info(1, "Secret App", "Secret Title", Some("Secret Body"))];

    let failures = survivor_failures(&captured, &after);

    assert_eq!(failures.len(), 1);
    assert!(!failures[0].contains("Secret"));
}

#[test]
fn an_action_that_removes_the_entry_counts_as_a_state_change() {
    let original = info(1, "App A", "Reminder", None);

    assert!(action_changed_state(
        &original,
        std::slice::from_ref(&original),
        &[]
    ));
}

#[test]
fn an_action_that_changes_the_entry_counts_as_a_state_change() {
    let original = info(1, "App A", "Reminder", Some("before"));
    let after = vec![info(1, "App A", "Reminder", Some("after"))];

    assert!(action_changed_state(
        &original,
        std::slice::from_ref(&original),
        &after
    ));
}

#[test]
fn an_unacknowledged_entry_left_unchanged_is_not_reported_as_a_change() {
    let original = info(1, "App A", "Reminder", Some("same"));
    let after = vec![info(1, "App A", "Reminder", Some("same"))];

    assert!(!action_changed_state(
        &original,
        std::slice::from_ref(&original),
        &after
    ));
}

#[test]
fn an_action_that_removes_one_of_two_identical_twins_counts_as_a_state_change() {
    let before = vec![
        info(1, "App A", "Reminder", Some("same")),
        info(2, "App A", "Reminder", Some("same")),
    ];
    let after = vec![info(1, "App A", "Reminder", Some("same"))];

    assert!(action_changed_state(&before[1], &before, &after));
    assert!(!action_changed_state(&before[1], &before, &before));
}

#[test]
fn a_target_departs_when_its_identical_entries_drop_by_the_targets_carrying_it() {
    let twin = info(1, "App A", "Ping", None);
    let other = info(3, "App B", "Other", None);
    let before = vec![twin.clone(), twin.clone(), other.clone()];
    let one_twin = std::slice::from_ref(&twin);

    assert!(departed(one_twin, &before, one_twin));
    assert!(!departed(one_twin, &before, &before));
    assert!(!departed(
        &[twin.clone(), twin.clone()],
        &before,
        &[twin.clone(), other.clone()]
    ));
    assert!(departed(&[twin.clone(), twin.clone()], &before, &[other]));
}

#[test]
fn a_surviving_dismiss_is_action_failed_and_delivered_unverified() {
    let error = dismiss_survived_error(3);

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert!(error.message.contains("index 3"));
    let suggestion = error.suggestion.as_deref().expect("a suggestion");
    assert!(
        suggestion.starts_with("Run list-notifications before deciding"),
        "{suggestion}"
    );
    assert!(
        !error.is_explicitly_retryable(),
        "the disposition is the retry contract: a delivered-unverified envelope serializes \
         retry-unsafe, so a contradictory retryable detail stamp must not ride beside it"
    );
}
