use super::{EditCommit, value_write_result, wrote_value_semantically};
use agent_desktop_core::{Action, ActionStep, StepMechanism};

fn headless_clear_steps() -> Vec<ActionStep> {
    vec![
        ActionStep::skipped("FocusThenClearByKeyboard")
            .with_mechanism(StepMechanism::PhysicalSynthetic),
        ActionStep::succeeded("AXValue")
            .with_mechanism(StepMechanism::SemanticApi)
            .with_verified(true),
    ]
}

#[test]
fn only_a_plain_text_field_waits_for_editing_to_end() {
    assert_eq!(
        EditCommit::from_roles(Some("AXTextField"), None),
        EditCommit::OnEndEditing
    );
    assert!(
        EditCommit::from_roles(Some("AXTextField"), None)
            .hint()
            .is_some_and(|hint| hint.contains("`focus`"))
    );
}

#[test]
fn a_search_field_secure_field_or_popup_is_settled_by_the_read_back() {
    for (role, subrole) in [
        ("AXTextField", Some("AXSearchField")),
        ("AXTextField", Some("AXSecureTextField")),
        ("AXSearchField", None),
        ("AXSecureTextField", None),
        ("AXPopUpButton", None),
        ("AXComboBox", None),
        ("AXSlider", None),
    ] {
        let commit = EditCommit::from_roles(Some(role), subrole);
        assert_eq!(commit, EditCommit::Immediate, "{role} {subrole:?}");
        assert_eq!(commit.hint(), None, "{role} {subrole:?}");
    }
}

#[test]
fn a_text_view_is_unobserved_and_settled_only_by_the_saved_result() {
    let commit = EditCommit::from_roles(Some("AXTextArea"), None);
    assert_eq!(commit, EditCommit::TextView);
    let hint = commit.hint().expect("text view hint");
    assert!(hint.contains("check the saved result"), "{hint}");
    assert_ne!(Some(hint), EditCommit::OnEndEditing.hint());
}

#[test]
fn an_unread_role_fails_closed_to_an_unobserved_commit() {
    assert!(EditCommit::Unread.hint().is_some());
    assert_eq!(EditCommit::from_roles(None, None), EditCommit::Immediate);
}

#[test]
fn a_semantic_clear_of_a_field_reports_an_unobserved_commit() {
    let result = value_write_result(&Action::Clear, headless_clear_steps(), || {
        EditCommit::OnEndEditing
    });

    assert!(result.commit_unobserved());
    assert_eq!(
        result.details.as_ref().unwrap()["commit_hint"],
        EditCommit::OnEndEditing.hint().unwrap()
    );
}

#[test]
fn a_semantic_clear_and_set_value_get_the_same_commit_report() {
    let steps = vec![
        ActionStep::succeeded("AXValue")
            .with_mechanism(StepMechanism::SemanticApi)
            .with_verified(true),
    ];
    let cleared = value_write_result(&Action::Clear, headless_clear_steps(), || {
        EditCommit::TextView
    });
    let written = value_write_result(&Action::SetValue(String::new()), steps, || {
        EditCommit::TextView
    });

    assert_eq!(cleared.details, written.details);
    assert_eq!(cleared.disposition(), written.disposition());
}

#[test]
fn a_keyboard_clear_is_judged_by_its_read_back() {
    let steps = vec![
        ActionStep::succeeded("FocusThenClearByKeyboard")
            .with_mechanism(StepMechanism::PhysicalSynthetic)
            .with_verified(false),
    ];
    let mut role_read = false;

    let result = value_write_result(&Action::Clear, steps, || {
        role_read = true;
        EditCommit::OnEndEditing
    });

    assert!(!role_read);
    assert!(!result.commit_unobserved());
}

#[test]
fn a_write_to_a_search_field_is_settled_by_its_read_back() {
    let result = value_write_result(&Action::Clear, headless_clear_steps(), || {
        EditCommit::Immediate
    });

    assert!(!result.commit_unobserved());
}

#[test]
fn only_a_delivered_semantic_step_counts_as_a_semantic_write() {
    assert!(wrote_value_semantically(&headless_clear_steps()));
    assert!(!wrote_value_semantically(&[
        ActionStep::skipped("AXValue").with_mechanism(StepMechanism::SemanticApi)
    ]));
}

#[test]
fn default_set_value_leaves_the_commit_to_the_caller() {
    for text in ["new", ""] {
        let result = super::set_value_result(
            &Action::SetValue(text.into()),
            headless_clear_steps(),
            || EditCommit::OnEndEditing,
            |_| panic!("default set-value must not end editing"),
        )
        .unwrap();
        assert!(result.commit_unobserved());
        let hint = result.details.unwrap()["commit_hint"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(hint.contains("--commit"));
        assert!(hint.contains("Return"));
    }
}

#[test]
fn opt_in_set_value_dispatches_the_commit_path() {
    let action = Action::SetValueAndCommit("new".into());
    let result = super::set_value_result(
        &action,
        headless_clear_steps(),
        || panic!("commit path reads its own role"),
        |steps| {
            Ok(
                agent_desktop_core::ActionResult::from_execution(&action, steps)
                    .with_commit_ended(serde_json::json!({"mechanism": "focus_round_trip"})),
            )
        },
    )
    .unwrap();
    assert_eq!(result.details.unwrap()["application_commit"], "committed");
}
