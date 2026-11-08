use super::*;
use agent_desktop_core::Modifier;

fn combo(key: &str, modifiers: Vec<Modifier>) -> KeyCombo {
    KeyCombo {
        key: key.into(),
        modifiers,
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
fn an_unreadable_menu_bar_falls_back_to_keys() {
    let error = AdapterError::new(ErrorCode::AppUnresponsive, "busy");
    assert_eq!(
        fallback_after(error, Deadline::standard().unwrap(), "lookup_incomplete").unwrap(),
        MenuShortcutOutcome::Skipped("lookup_incomplete")
    );
    let error = AdapterError::new(ErrorCode::Timeout, "lookup budget spent");
    assert_eq!(
        fallback_after(error, Deadline::standard().unwrap(), "lookup_incomplete").unwrap(),
        MenuShortcutOutcome::Skipped("lookup_incomplete")
    );
}

#[test]
fn a_permission_failure_is_not_hidden_behind_the_keyboard_path() {
    let error = AdapterError::new(ErrorCode::PermDenied, "no accessibility");
    assert_eq!(
        fallback_after(error, Deadline::standard().unwrap(), "lookup_incomplete")
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
        fallback_after(error, expired, "lookup_incomplete")
            .unwrap_err()
            .code,
        ErrorCode::Timeout
    );
}
