use super::windows_from_records_with_focus;
use crate::system::window::cg_window::WindowRecord;
use crate::system::window::window_ax_state::WindowAxState;
use crate::system::window::window_ax_titles::WindowAxTitles;
use agent_desktop_core::AdapterError;
use rustc_hash::{FxHashMap, FxHashSet};

fn untitled(window_number: i64) -> WindowRecord {
    WindowRecord {
        app_name: "Fixture".into(),
        pid: 10,
        title: None,
        window_number,
        bounds: agent_desktop_core::Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        },
        presence: crate::system::window::window_presence::WindowPresence::Onscreen,
        process_instance: Some("instance-10".into()),
    }
}

fn state(titles: WindowAxTitles, focused_number: Option<i64>) -> WindowAxState {
    WindowAxState {
        focused: focused_number.map(|number| (None, Some(number))),
        minimized_by_id: FxHashMap::default(),
        accessible_window_ids: FxHashSet::from_iter([7, 8]),
        titles,
    }
}

fn titles(entries: &[(i64, &str)]) -> WindowAxTitles {
    let mut titles = WindowAxTitles::default();
    for (number, title) in entries {
        titles.record(*number, Some((*title).to_owned()));
    }
    titles
}

#[test]
fn a_window_without_a_cg_name_reports_its_bridged_ax_title() {
    let windows = windows_from_records_with_focus(
        vec![untitled(7), untitled(8)],
        false,
        |_| Ok(state(titles(&[(7, "Alpha"), (8, "Beta")]), Some(8))),
        |_, _| Ok(true),
    )
    .unwrap();

    assert_eq!(windows[0].title, "Alpha");
    assert_eq!(windows[1].title, "Beta");
    assert!(windows[1].state.is_focused);
}

#[test]
fn an_ambiguous_ax_join_leaves_the_title_empty_not_the_app_name() {
    let windows = windows_from_records_with_focus(
        vec![untitled(7)],
        false,
        |_| Ok(state(titles(&[(7, "Alpha"), (7, "Beta")]), None)),
        |_, _| Ok(true),
    )
    .unwrap();

    assert_eq!(windows[0].title, "");
}

#[test]
fn a_window_with_no_ax_window_has_an_empty_title_not_the_app_name() {
    let windows = windows_from_records_with_focus(
        vec![untitled(7)],
        false,
        |_| Ok(state(titles(&[(8, "Other")]), None)),
        |_, _| Ok(true),
    )
    .unwrap();

    assert_eq!(windows[0].title, "");
    assert_ne!(windows[0].title, windows[0].app);
}

#[test]
fn an_unreadable_ax_state_leaves_the_title_empty_not_the_app_name() {
    let windows = windows_from_records_with_focus(
        vec![untitled(7)],
        false,
        |_| Err(AdapterError::timeout("busy")),
        |_, _| Ok(true),
    )
    .unwrap();

    assert_eq!(windows[0].title, "");
}
