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

#[test]
fn a_background_apps_disabled_reading_does_not_block_the_press() {
    assert_eq!(pressable_flags(&[found(false)], false), [true]);
    assert_eq!(
        pressable_flags(&[found(false), found(true)], false),
        [true, true]
    );
}

#[test]
fn the_frontmost_apps_disabled_reading_is_trusted() {
    assert_eq!(pressable_flags(&[found(false)], true), [false]);
    assert_eq!(
        pressable_flags(&[found(false), found(true)], true),
        [false, true]
    );
}

#[test]
fn a_press_in_a_background_app_says_its_enabled_state_was_not_validated() {
    let details = pressed_details(&found(false), false, &[]);
    assert_eq!(
        details,
        serde_json::json!({ "menu": "File", "item": "New Folder", "enabled": "unvalidated" })
    );
    let details = pressed_details(&found(true), true, &["Edit > AutoFill".into()]);
    assert_eq!(
        details,
        serde_json::json!({
            "menu": "File", "item": "New Folder", "enabled": true,
            "unread_menus": ["Edit > AutoFill"]
        })
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
