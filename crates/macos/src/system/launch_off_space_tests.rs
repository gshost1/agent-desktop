use super::{OFF_SPACE_NOTE, explain_off_space, has_off_space_window};
use crate::system::window::cg_window::WindowRecord;
use agent_desktop_core::{
    Deadline, ProcessId, Rect, system::launch_notes::LaunchNotes,
    system::launch_result::LaunchResult,
};

fn record(visible: bool, width: f64, height: f64) -> WindowRecord {
    WindowRecord {
        app_name: "probe".into(),
        pid: 7,
        title: None,
        window_number: 41,
        bounds: Rect {
            x: 200.0,
            y: 200.0,
            width,
            height,
        },
        visible,
        process_instance: None,
    }
}

fn windowless(suggestion: Option<String>) -> LaunchResult {
    LaunchResult {
        app: "probe".into(),
        pid: ProcessId::new(7),
        process_instance: None,
        window: None,
        cdp: None,
        renderer: None,
        notes: LaunchNotes {
            suggestion,
            launch_path: None,
        },
    }
}

#[test]
fn an_off_screen_window_with_a_real_size_is_off_space() {
    assert!(has_off_space_window(&[record(false, 230.0, 408.0)]));
}

#[test]
fn an_on_screen_window_is_not_off_space() {
    assert!(!has_off_space_window(&[record(true, 230.0, 408.0)]));
}

#[test]
fn a_window_still_being_created_is_not_off_space() {
    assert!(!has_off_space_window(&[record(false, 0.0, 0.0)]));
    assert!(!has_off_space_window(&[record(false, 230.0, 0.0)]));
}

#[test]
fn no_window_record_is_not_off_space() {
    assert!(!has_off_space_window(&[]));
}

#[test]
fn a_result_that_already_carries_a_note_keeps_it() {
    let deadline = Deadline::after(1_000).unwrap();
    let mut noted = windowless(Some("several windows".into()));
    explain_off_space(&mut noted, 7, deadline);
    assert_eq!(noted.notes.suggestion.as_deref(), Some("several windows"));
}

#[test]
fn the_note_names_the_activate_flag_and_the_error_agents_will_see() {
    assert!(OFF_SPACE_NOTE.contains("--activate"));
    assert!(OFF_SPACE_NOTE.contains("WINDOW_NOT_FOUND"));
}
