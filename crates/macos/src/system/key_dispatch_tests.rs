use super::*;
use agent_desktop_core::Modifier;

#[test]
fn process_preflight_failure_reports_no_key_delivery() {
    let error = press_for_app_impl(
        ProcessIdentity::new(u32::MAX, "invalid"),
        &combo(Vec::new()),
        agent_desktop_core::InteractionPolicy::default(),
        Deadline::standard().unwrap(),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidArgs);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

fn combo(modifiers: Vec<Modifier>) -> KeyCombo {
    KeyCombo {
        key: "a".into(),
        modifiers,
    }
}

fn key(name: &str, modifiers: Vec<Modifier>) -> KeyCombo {
    KeyCombo {
        key: name.into(),
        modifiers,
    }
}

#[test]
fn printable_keys_are_never_translated_into_ax_actions() {
    for name in ["space", "a", "1", "tab", "delete"] {
        assert_eq!(simple_key_action(&key(name, Vec::new())), None, "{name}");
    }
}

#[test]
fn return_and_escape_map_to_default_and_cancel_actions() {
    assert_eq!(
        simple_key_action(&key("return", Vec::new())),
        Some("AXConfirm")
    );
    assert_eq!(
        simple_key_action(&key("enter", Vec::new())),
        Some("AXConfirm")
    );
    assert_eq!(
        simple_key_action(&key("escape", Vec::new())),
        Some("AXCancel")
    );
    assert_eq!(
        simple_key_action(&key("return", vec![Modifier::Meta])),
        None
    );
}

#[test]
fn command_chords_try_the_menu_bar_under_either_policy() {
    use agent_desktop_core::InteractionPolicy;
    for policy in [InteractionPolicy::headless(), InteractionPolicy::headed()] {
        assert!(uses_menu_shortcut(&key("n", vec![Modifier::Meta]), policy));
        assert!(uses_menu_shortcut(
            &key("n", vec![Modifier::Meta, Modifier::Shift]),
            policy
        ));
    }
}

#[test]
fn chords_without_command_stay_on_the_keyboard_path() {
    use agent_desktop_core::InteractionPolicy;
    for policy in [InteractionPolicy::headless(), InteractionPolicy::headed()] {
        assert!(!uses_menu_shortcut(&key("n", Vec::new()), policy));
        assert!(!uses_menu_shortcut(&key("x", vec![Modifier::Ctrl]), policy));
        assert!(!uses_menu_shortcut(
            &key("tab", vec![Modifier::Alt]),
            policy
        ));
        assert!(!uses_menu_shortcut(
            &key("return", vec![Modifier::Shift]),
            policy
        ));
    }
}

#[test]
fn a_pressed_menu_item_is_reported_as_its_own_semantic_step() {
    let report = menu_shortcut_report(&MenuShortcutOutcome::Pressed {
        menu: Some("File".into()),
        item: Some("New Folder".into()),
    });
    assert_eq!(report.steps.len(), 1);
    assert_eq!(report.steps[0].label(), MENU_SHORTCUT_STEP);
    assert_eq!(
        report.steps[0].mechanism(),
        Some(StepMechanism::SemanticApi)
    );
    assert_eq!(report.steps[0].verified(), Some(false));
    assert_eq!(
        report.details,
        Some(serde_json::json!({
            "menu_shortcut": { "menu": "File", "item": "New Folder" }
        }))
    );
}

#[test]
fn a_skipped_lookup_names_its_reason_before_the_keystroke() {
    let report = menu_shortcut_report(&MenuShortcutOutcome::Skipped("ambiguous"));
    assert_eq!(report.steps.len(), 1);
    assert_eq!(report.steps[0].label(), MENU_SHORTCUT_STEP);
    assert!(matches!(
        report.steps[0].outcome,
        agent_desktop_core::ActionStepOutcome::Skipped
    ));
    assert_eq!(
        report.details,
        Some(serde_json::json!({ "menu_shortcut": { "skipped": "ambiguous" } }))
    );
}

#[test]
fn an_inapplicable_lookup_leaves_the_keyboard_report_unchanged() {
    let report = menu_shortcut_report(&MenuShortcutOutcome::NotApplicable);
    assert!(report.steps.is_empty());
    assert!(report.details.is_none());
}

#[test]
fn a_missing_focused_element_names_both_recovery_routes() {
    let error = super::no_focused_element_error(agent_desktop_core::InteractionPolicy::headless());
    let suggestion = error
        .suggestion
        .expect("the error carries a recovery suggestion");
    assert!(suggestion.contains("--headed"), "{suggestion}");
    assert!(suggestion.contains("--surface menubar"), "{suggestion}");
}

#[test]
fn a_headed_missing_focused_element_does_not_suggest_enabling_headed() {
    let error = super::no_focused_element_error(agent_desktop_core::InteractionPolicy::headed());
    let hint = error.suggestion.expect("recovery suggestion");
    assert!(!hint.contains("--headed"), "{hint}");
    assert!(hint.contains("--surface menubar"), "{hint}");
}

#[test]
fn an_unresolvable_key_fails_before_any_focus_change() {
    let pid = std::process::id();
    let instance = crate::system::process::process_identity::token_for_pid(pid as i32)
        .unwrap()
        .unwrap();
    let error = press_for_app_impl(
        ProcessIdentity::new(pid, instance),
        &key("comma", Vec::new()),
        agent_desktop_core::InteractionPolicy::headed(),
        Deadline::standard().unwrap(),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(
        error.message.contains("keyboard layout"),
        "{}",
        error.message
    );
}
