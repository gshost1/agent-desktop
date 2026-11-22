use super::*;
use crate::system::window::window_presence::WindowPresence;

fn record(presence: WindowPresence) -> WindowRecord {
    WindowRecord {
        app_name: "Fixture".into(),
        pid: 10,
        title: Some("Fixture".into()),
        window_number: 42,
        bounds: agent_desktop_core::Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        },
        presence,
        process_instance: Some("instance-10".into()),
    }
}

fn state(published: bool, complete: bool) -> WindowAxState {
    WindowAxState {
        focused: None,
        minimized_by_id: rustc_hash::FxHashMap::default(),
        accessible_window_ids: if published {
            rustc_hash::FxHashSet::from_iter([42])
        } else {
            rustc_hash::FxHashSet::default()
        },
        window_numbers_complete: complete,
        titles: Default::default(),
    }
}

#[test]
fn an_unpublished_offscreen_window_on_no_space_is_closed() {
    let error = require_live_window(
        "w-42",
        &record(WindowPresence::Offscreen),
        &state(false, true),
        |_| Some(false),
    )
    .unwrap_err();
    assert_eq!(error.code, agent_desktop_core::ErrorCode::WindowNotFound);
    assert_eq!(error.message, "Window 'w-42' not found");
}

#[test]
fn an_unpublished_window_on_another_space_is_not_closed() {
    assert!(!is_ordered_out(
        &record(WindowPresence::Offscreen),
        &state(false, true),
        |_| Some(true)
    ));
}

#[test]
fn an_unknown_space_keeps_the_window() {
    assert!(!is_ordered_out(
        &record(WindowPresence::Offscreen),
        &state(false, true),
        |_| None
    ));
}

#[test]
fn an_incomplete_window_set_keeps_the_window() {
    assert!(!is_ordered_out(
        &record(WindowPresence::Offscreen),
        &state(false, false),
        |_| Some(false)
    ));
}

#[test]
fn a_published_hidden_or_minimized_window_is_not_closed() {
    assert!(!is_ordered_out(
        &record(WindowPresence::Offscreen),
        &state(true, true),
        |_| Some(false)
    ));
}

#[test]
fn an_onscreen_window_is_never_closed_and_skips_the_space_read() {
    for presence in [WindowPresence::Onscreen, WindowPresence::Floating] {
        assert!(!is_ordered_out(
            &record(presence),
            &state(false, true),
            |_| panic!("an onscreen window needs no Space read"),
        ));
    }
}
