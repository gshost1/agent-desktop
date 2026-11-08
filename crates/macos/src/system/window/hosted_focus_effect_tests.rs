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
        visible,
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
