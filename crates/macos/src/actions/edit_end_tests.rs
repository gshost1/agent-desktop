use super::*;

#[test]
fn text_areas_require_exactly_one_observed_show_menu_action() {
    assert!(safe_text_area(Some(&["AXShowMenu".into()])));
    for names in [
        None,
        Some(vec![]),
        Some(vec!["AXConfirm".into()]),
        Some(vec!["AXShowMenu".into(), "AXPress".into()]),
    ] {
        assert!(!safe_text_area(names.as_deref()));
    }
}

#[test]
fn only_non_controls_or_safe_text_areas_may_receive_delegated_focus() {
    for role in ["AXWindow", "AXGroup", "AXScrollArea", "AXStaticText"] {
        assert!(safe_focused_role(Some(role), None));
    }
    assert!(safe_focused_role(
        Some("AXTextArea"),
        Some(&["AXShowMenu".into()])
    ));
    for role in [
        None,
        Some("AXTextField"),
        Some("AXTable"),
        Some("AXButton"),
        Some("AXComboBox"),
        Some("AXMenu"),
        Some("AXImage"),
    ] {
        assert!(!safe_focused_role(role, None));
    }
    assert!(!safe_focused_role(Some("AXTextArea"), None));
    assert!(!safe_focused_role(
        Some("AXTextArea"),
        Some(&["AXPress".into()])
    ));
}

#[test]
fn containers_with_focusable_or_unread_controls_are_excluded() {
    for role in [
        "AXGroup",
        "AXScrollArea",
        "AXStaticText",
        "AXImage",
        "AXTextArea",
    ] {
        assert!(safe_subtree_node(Some(role), Some(true)));
    }
    for role in [
        None,
        Some("AXTextField"),
        Some("AXTable"),
        Some("AXButton"),
        Some("AXComboBox"),
    ] {
        assert!(!safe_subtree_node(role, Some(true)));
        assert!(!safe_subtree_node(role, None));
        assert!(safe_subtree_node(role, Some(false)));
    }
}

#[test]
fn confirm_requires_focus_loss_or_a_dependent_change_not_just_success() {
    assert!(confirm_evidence(Some(false), false));
    assert!(confirm_evidence(Some(true), true));
    assert!(confirm_evidence(None, true));
    assert!(!confirm_evidence(Some(true), false));
    assert!(!confirm_evidence(None, false));
}

#[test]
fn ordered_mechanisms_stop_at_the_first_ended_edit() {
    let candidates = [
        ("focus_off", 0),
        ("container_focus", 1),
        ("text_area_focus", 2),
        ("confirm", 3),
    ];
    for end in 0..4 {
        let mut seen = Vec::new();
        let (result, mechanism) = ordered_attempts(candidates, |_, value| {
            seen.push(value);
            if value == end {
                Trial::Ended(vec![])
            } else {
                Trial::Continue
            }
        });
        assert!(matches!(result, EndedEdit::Committed(_)));
        assert_eq!(mechanism, Some(candidates[end].0));
        assert_eq!(seen, (0..=end).collect::<Vec<_>>());
    }
    let (result, _) = ordered_attempts(candidates, |_, _| Trial::Continue);
    let EndedEdit::NoSafePeer(details) = result else {
        panic!("expected refusal");
    };
    assert_eq!(
        details["attempted"],
        json!(["focus_off", "container_focus", "text_area_focus", "confirm"])
    );
    assert_eq!(details["confirm_sent"], true);
}

#[test]
fn empty_or_failed_attempts_do_not_claim_confirm_was_sent() {
    for candidates in [
        vec![],
        vec![
            ("focus_off", 0),
            ("container_focus", 1),
            ("container_focus", 2),
        ],
    ] {
        let (result, _) = ordered_attempts(candidates, |_, _| Trial::Continue);
        let EndedEdit::NoSafePeer(details) = result else {
            panic!("expected refusal");
        };
        assert_eq!(details["confirm_sent"], false);
        assert!(details["attempted"].as_array().unwrap().len() <= 2);
    }
}

#[test]
fn delegation_and_app_refusal_stop_before_confirm() {
    for delegated in [true, false] {
        let mut calls = 0;
        let (result, _) = ordered_attempts([("container_focus", ()), ("confirm", ())], |_, _| {
            calls += 1;
            if delegated {
                Trial::Delegated
            } else {
                Trial::Refused
            }
        });
        assert_eq!(calls, 1);
        assert!(if delegated {
            matches!(result, EndedEdit::Delegated)
        } else {
            matches!(result, EndedEdit::Refused)
        });
    }
}

#[test]
fn both_new_errors_are_delivered_failures_with_recovery_and_commit_status() {
    for (error, kind, commit) in [
        (
            no_safe_peer_error(json!({"attempted": ["confirm"], "confirm_sent": true})),
            "edit_commit_no_safe_peer",
            "unobserved",
        ),
        (
            focus_delegated_error(),
            "edit_commit_focus_delegated",
            "not_verified",
        ),
    ] {
        assert_eq!(error.code, ErrorCode::ActionFailed);
        assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
        let details = error.details.unwrap();
        assert_eq!(details["kind"], kind);
        assert_eq!(details["application_commit"], commit);
        assert!(error.suggestion.is_some());
        if kind == "edit_commit_no_safe_peer" {
            assert_eq!(details["attempted"], json!(["confirm"]));
            assert_eq!(details["confirm_sent"], true);
            assert!(error.suggestion.unwrap().contains("Do not press Return"));
        }
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
