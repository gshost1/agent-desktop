use super::menu_match::MenuMatch;
use super::*;
use agent_desktop_core::Modifier;

fn combo(key: &str, modifiers: Vec<Modifier>) -> KeyCombo {
    KeyCombo {
        key: key.into(),
        modifiers,
    }
}

fn found(enabled: bool) -> MenuMatch {
    MenuMatch {
        element: crate::tree::element_for_pid(i32::MAX),
        menu: Some("File".into()),
        item: Some("New Folder".into()),
        enabled,
    }
}

#[test]
fn chords_without_command_never_read_the_menu_bar() {
    let process = ProcessIdentity::new(u32::MAX, "invalid");
    let app = crate::tree::element_for_pid(i32::MAX);
    for chord in [
        combo("return", Vec::new()),
        combo("a", Vec::new()),
        combo("x", vec![Modifier::Ctrl]),
        combo("tab", vec![Modifier::Alt, Modifier::Shift]),
    ] {
        assert_eq!(
            press_menu_shortcut(&process, &app, &chord, Deadline::standard().unwrap()).unwrap(),
            MenuShortcutOutcome::NotApplicable
        );
    }
}

fn walk_with(matches: Vec<MenuMatch>) -> menu_walk::MenuWalk {
    menu_walk::MenuWalk {
        matches,
        visited: 9,
        ..menu_walk::MenuWalk::default()
    }
}

#[test]
fn a_disabled_item_in_a_background_app_is_refused_as_not_delivered() {
    let error = disabled_in_background(&walk_with(vec![found(false)]));
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    let details = error.details.unwrap();
    assert_eq!(details["kind"], "menu_item_disabled_in_background");
    assert_eq!(details["physical_delivery_started"], false);
    assert_eq!(
        details["menu_shortcut"],
        serde_json::json!({
            "menu": "File", "item": "New Folder", "enabled": false, "frontmost": false, "visited": 9
        })
    );
    assert!(error.suggestion.unwrap().contains("--headed"));
}

#[test]
fn only_disabled_matches_refuse_in_the_background_and_fall_back_in_front() {
    let walk = walk_with(vec![found(false)]);
    let error = only_disabled_matches(&walk, false).unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(
        only_disabled_matches(&walk, true).unwrap(),
        MenuShortcutOutcome::Skipped(serde_json::json!({ "skipped": "disabled", "visited": 9 }))
    );
}

#[test]
fn a_skip_carries_the_walk_report() {
    let mut walk = walk_with(Vec::new());
    walk.unread_menus.push("Edit > AutoFill".into());
    assert_eq!(
        skipped_after(&walk, "ambiguous"),
        MenuShortcutOutcome::Skipped(serde_json::json!({
            "skipped": "ambiguous", "visited": 9, "unread_menus": ["Edit > AutoFill"]
        }))
    );
}

#[test]
fn an_unreadable_menu_bar_falls_back_and_says_where_it_stopped() {
    let error = AdapterError::new(ErrorCode::AppUnresponsive, "busy").with_details(
        serde_json::json!({ "kind": "child_read", "menu": "Edit", "depth": 2, "visited": 41, "other": 1 }),
    );
    let MenuShortcutOutcome::Skipped(details) =
        fallback_after(error, Deadline::standard().unwrap(), "menu_walk").unwrap()
    else {
        panic!("expected a skipped lookup");
    };
    assert_eq!(
        details,
        serde_json::json!({
            "skipped": "lookup_incomplete", "stage": "menu_walk", "code": "APP_UNRESPONSIVE",
            "kind": "child_read", "menu": "Edit", "depth": 2, "visited": 41
        })
    );
}

#[test]
fn a_spent_lookup_budget_falls_back() {
    let error = AdapterError::new(ErrorCode::Timeout, "lookup budget spent");
    assert!(matches!(
        fallback_after(error, Deadline::standard().unwrap(), "menu_walk").unwrap(),
        MenuShortcutOutcome::Skipped(_)
    ));
}

#[test]
fn a_permission_failure_is_not_hidden_behind_the_keyboard_path() {
    let error = AdapterError::new(ErrorCode::PermDenied, "no accessibility");
    assert_eq!(
        fallback_after(error, Deadline::standard().unwrap(), "menu_walk")
            .unwrap_err()
            .code,
        ErrorCode::PermDenied
    );
}

#[test]
fn an_exhausted_command_deadline_surfaces_instead_of_falling_back() {
    let expired = Deadline::from_duration(std::time::Duration::ZERO).unwrap();
    let error = AdapterError::new(ErrorCode::AppUnresponsive, "busy");
    assert_eq!(
        fallback_after(error, expired, "menu_walk")
            .unwrap_err()
            .code,
        ErrorCode::Timeout
    );
}
