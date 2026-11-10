use super::*;

fn window(id: &str, focused: bool) -> WindowInfo {
    WindowInfo {
        id: id.into(),
        title: id.into(),
        app: "probe".into(),
        pid: agent_desktop_core::ProcessId::new(7),
        process_instance: Some("instance".into()),
        bounds: None,
        state: agent_desktop_core::WindowState {
            is_focused: focused,
            ..Default::default()
        },
    }
}

#[test]
fn the_only_window_is_claimed() {
    assert!(matches!(
        LaunchWindow::pick(vec![window("w-1", false)]),
        LaunchWindow::Shown(shown) if shown.id == "w-1"
    ));
}

#[test]
fn the_only_focused_window_among_several_is_claimed() {
    assert!(matches!(
        LaunchWindow::pick(vec![window("w-1", false), window("w-2", true)]),
        LaunchWindow::Shown(shown) if shown.id == "w-2"
    ));
}

#[test]
fn several_windows_without_one_focused_claim_none_and_name_each() {
    for focus in [false, true] {
        let picked = LaunchWindow::pick(vec![window("w-1", focus), window("w-2", focus)]);
        assert!(matches!(&picked, LaunchWindow::Several(ids) if ids == &["w-1", "w-2"]));
        let (shown, note) = picked.into_parts();
        assert!(shown.is_none());
        assert!(note.is_some_and(|note| note.contains("w-1, w-2")));
    }
}

#[test]
fn no_window_is_absent_with_no_note() {
    let (shown, note) = LaunchWindow::pick(Vec::new()).into_parts();
    assert!(shown.is_none());
    assert!(note.is_none());
}
