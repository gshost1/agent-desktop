use super::{changed_values, nearest_peer, refused_commit_error, takes_focus_without_acting};

#[test]
fn only_a_text_field_that_is_not_secure_or_disabled_takes_focus_for_the_commit() {
    assert!(takes_focus_without_acting(
        Some("AXTextField"),
        None,
        Some(true)
    ));
    assert!(takes_focus_without_acting(
        Some("AXTextField"),
        Some("AXSearchField"),
        None
    ));
    for (role, subrole, enabled) in [
        (Some("AXTextField"), Some("AXSecureTextField"), Some(true)),
        (Some("AXTextField"), None, Some(false)),
        (Some("AXButton"), None, Some(true)),
        (Some("AXPopUpButton"), None, Some(true)),
        (Some("AXMenuItem"), None, Some(true)),
        (Some("AXCheckBox"), None, Some(true)),
        (Some("AXSlider"), None, Some(true)),
        (Some("AXTextArea"), None, Some(true)),
        (None, None, None),
    ] {
        assert!(
            !takes_focus_without_acting(role, subrole, enabled),
            "{role:?} {subrole:?}"
        );
    }
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
fn only_neighbours_whose_value_changed_are_reported_up_to_three() {
    let before: Vec<(String, Option<String>)> = (0..6)
        .map(|index| ("slider".to_string(), Some(index.to_string())))
        .collect();
    let mut after: Vec<Option<String>> = before.iter().map(|(_, value)| value.clone()).collect();
    assert!(changed_values(&before, &after).is_empty());
    after[1] = Some("0.5".into());
    let changes = changed_values(&before, &after);
    assert_eq!(changes.len(), 1);
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
