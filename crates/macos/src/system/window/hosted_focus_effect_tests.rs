use super::*;
use agent_desktop_core::Rect;
use core_foundation::string::CFString;

fn window(window_number: i64, width: f64, visible: bool) -> WindowRecord {
    WindowRecord {
        app_name: "Host".into(),
        pid: 11_741,
        title: None,
        window_number,
        bounds: Rect {
            x: 28.0,
            y: 169.0,
            width,
            height: 448.0,
        },
        presence: crate::system::window::window_presence::WindowPresence::from_onscreen(visible),
        process_instance: None,
    }
}

fn signature(state: Option<&str>, windows: Vec<WindowRecord>) -> FocusSignature {
    FocusSignature {
        focused: None,
        state: state.map(|value| CFString::new(value).as_CFType()),
        windows,
    }
}

#[test]
fn an_unchanged_focus_state_and_window_set_is_no_effect() {
    let before = signature(Some("Untitled"), vec![window(1, 880.0, true)]);
    let after = signature(Some("Untitled"), vec![window(1, 880.0, true)]);

    assert!(!after.differs_from(&before));
}

#[test]
fn a_new_window_from_the_panel_is_an_effect() {
    let before = signature(Some("Untitled"), vec![window(1, 880.0, true)]);
    let after = signature(
        Some("Untitled"),
        vec![window(1, 880.0, true), window(2, 460.0, true)],
    );

    assert!(after.differs_from(&before));
}

#[test]
fn a_closed_or_hidden_window_is_an_effect() {
    let before = signature(Some("Untitled"), vec![window(1, 880.0, true)]);

    assert!(signature(Some("Untitled"), Vec::new()).differs_from(&before));
    assert!(signature(Some("Untitled"), vec![window(1, 880.0, false)]).differs_from(&before));
}

#[test]
fn a_changed_focused_value_or_selection_is_an_effect() {
    let before = signature(Some("Untitled"), vec![window(1, 880.0, true)]);

    assert!(signature(Some("Untitled2"), vec![window(1, 880.0, true)]).differs_from(&before));
    assert!(signature(None, vec![window(1, 880.0, true)]).differs_from(&before));
}

#[test]
fn a_keyboard_target_keeps_the_details_the_route_already_reported() {
    let result = ActionResult::delivered_unverified("press_key").with_details(serde_json::json!({
        "menu_shortcut": { "skipped": "ambiguous" },
        "modal_key": { "effect": "observed" },
    }));
    let target = serde_json::json!({ "pid": 2, "route": "hosted_view" });
    let result = with_target(result, Some(target.clone()));
    assert_eq!(
        result.details,
        Some(serde_json::json!({
            "menu_shortcut": { "skipped": "ambiguous" },
            "modal_key": { "effect": "observed" },
            "keyboard_target": target,
        }))
    );
}

#[test]
fn a_keyboard_target_on_a_bare_result_is_its_only_detail() {
    let target = serde_json::json!({ "pid": 2 });
    let result = with_target(
        ActionResult::delivered_unverified("press_key"),
        Some(target.clone()),
    );
    assert_eq!(
        result.details,
        Some(serde_json::json!({ "keyboard_target": target }))
    );
    let untouched = with_target(ActionResult::delivered_unverified("press_key"), None);
    assert_eq!(untouched.details, None);
}
