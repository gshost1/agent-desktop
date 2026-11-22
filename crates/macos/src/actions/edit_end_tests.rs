use super::{
    EndedEdit, changed_values, focus_not_restored_error, nearest_peer, refused_commit_error,
    reports_value, settle_round_trip, takes_focus_without_acting,
};

#[test]
fn only_an_enabled_plain_text_field_with_every_fact_read_is_a_focus_peer() {
    assert!(takes_focus_without_acting(
        Some("AXTextField"),
        Some(None),
        Some(true),
        Some(true)
    ));
    for (role, subrole, enabled, settable) in [
        (
            Some("AXTextField"),
            Some(Some("AXSearchField")),
            Some(true),
            Some(true),
        ),
        (
            Some("AXTextField"),
            Some(Some("AXSecureTextField")),
            Some(true),
            Some(true),
        ),
        (Some("AXTextField"), None, Some(true), Some(true)),
        (Some("AXTextField"), Some(None), None, Some(true)),
        (Some("AXTextField"), Some(None), Some(false), Some(true)),
        (Some("AXTextField"), Some(None), Some(true), None),
        (Some("AXTextField"), Some(None), Some(true), Some(false)),
        (Some("AXButton"), Some(None), Some(true), Some(true)),
        (Some("AXPopUpButton"), Some(None), Some(true), Some(true)),
        (Some("AXMenuItem"), Some(None), Some(true), Some(true)),
        (Some("AXTextArea"), Some(None), Some(true), Some(true)),
        (None, Some(None), Some(true), Some(true)),
    ] {
        assert!(
            !takes_focus_without_acting(role, subrole, enabled, settable),
            "{role:?} {subrole:?} {enabled:?} {settable:?}"
        );
    }
}

#[test]
fn a_round_trip_commits_only_when_focus_left_and_came_back() {
    let evidence = || serde_json::json!({ "mechanism": "focus_round_trip" });
    assert!(matches!(
        settle_round_trip(Some(true), Some(false), Some(true), evidence()),
        EndedEdit::Committed(_)
    ));
    for back in [Some(false), None] {
        assert!(matches!(
            settle_round_trip(Some(true), Some(false), back, evidence()),
            EndedEdit::NotRestored(_)
        ));
    }
    for (peer, kept) in [
        (Some(false), Some(false)),
        (None, Some(false)),
        (Some(true), Some(true)),
        (Some(true), None),
    ] {
        assert!(matches!(
            settle_round_trip(peer, kept, Some(true), evidence()),
            EndedEdit::Refused
        ));
    }
}

#[test]
fn focus_that_did_not_come_back_is_a_delivered_failure_naming_the_recovery() {
    let error = focus_not_restored_error(serde_json::json!({ "focus_returned": false }));
    assert_eq!(error.code, agent_desktop_core::ErrorCode::ActionFailed);
    assert_eq!(
        error.disposition,
        agent_desktop_core::DeliverySemantics::delivered_unverified()
    );
    let details = error.details.as_ref().expect("details");
    assert_eq!(details["kind"], "edit_focus_not_restored");
    assert_ne!(details["application_commit"], "committed");
    assert!(
        error
            .suggestion
            .as_deref()
            .is_some_and(|hint| hint.contains("focus <ref>"))
    );
}

#[test]
fn a_secure_or_unread_neighbour_is_never_read_as_evidence() {
    assert!(reports_value("AXSlider", Some(None)));
    assert!(reports_value("AXTextField", Some(Some("AXSearchField"))));
    assert!(!reports_value(
        "AXTextField",
        Some(Some("AXSecureTextField"))
    ));
    assert!(!reports_value("AXSecureTextField", Some(None)));
    assert!(!reports_value("AXTextField", None));
}

fn tree(node: &u32) -> Vec<u32> {
    match node {
        1 => vec![10, 11],
        10 => vec![100, 101],
        11 => vec![110],
        2 => vec![1, 20],
        20 => vec![200],
        _ => vec![],
    }
}

#[test]
fn the_nearest_peer_comes_from_the_closest_ancestor_and_never_from_the_field() {
    let peers = [101, 110, 200];
    let found = nearest_peer(
        &[10, 1, 2],
        |node| *node == 100,
        tree,
        |node| peers.contains(node),
    );
    assert_eq!(found, Some(101));

    let found = nearest_peer(&[10, 1, 2], |node| *node == 100, tree, |node| *node == 200);
    assert_eq!(found, Some(200));

    let found = nearest_peer(&[10, 1, 2], |node| *node == 100, tree, |node| *node == 100);
    assert_eq!(found, None, "the written field is never its own peer");
}

#[test]
fn a_field_nested_under_the_written_field_is_not_searched() {
    let found = nearest_peer(&[1], |node| *node == 10, tree, |node| *node == 101);
    assert_eq!(found, None);
}

#[test]
fn only_neighbours_read_on_both_sides_with_a_different_value_are_reported_up_to_three() {
    let before: Vec<(String, String)> = (0..6)
        .map(|index| ("slider".to_string(), index.to_string()))
        .collect();
    let mut after: Vec<Option<String>> = before
        .iter()
        .map(|(_, value)| Some(value.clone()))
        .collect();
    assert!(changed_values(&before, &after).is_empty());
    after[1] = Some("0.5".into());
    after[2] = None;
    let changes = changed_values(&before, &after);
    assert_eq!(
        changes.len(),
        1,
        "an unread after-value is not a change: {changes:?}"
    );
    assert_eq!(changes[0]["role"], "slider");
    assert_eq!(changes[0]["before"], "1");
    assert_eq!(changes[0]["after"], "0.5");
    for value in after.iter_mut() {
        *value = Some("x".into());
    }
    assert_eq!(changed_values(&before, &after).len(), 3);
}

#[test]
fn a_refused_end_of_edit_is_a_delivered_failure_that_points_at_the_alert() {
    let error = refused_commit_error();
    assert_eq!(error.code, agent_desktop_core::ErrorCode::ActionFailed);
    assert_eq!(
        error.disposition,
        agent_desktop_core::DeliverySemantics::delivered_unverified()
    );
    let details = error.details.as_ref().expect("details");
    assert_eq!(details["kind"], "edit_commit_refused");
    assert_eq!(details["application_commit"], "refused");
    assert!(
        error
            .suggestion
            .as_deref()
            .is_some_and(|hint| hint.contains("--surface alert")),
        "{:?}",
        error.suggestion
    );
}
