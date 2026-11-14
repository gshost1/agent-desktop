use super::*;

fn window(id: &str, focused: bool) -> WindowInfo {
    WindowInfo {
        id: id.into(),
        title: id.into(),
        app: "probe".into(),
        pid: agent_desktop_core::ProcessId::new(7),
        process_instance: Some("instance".into()),
        bounds: Some(agent_desktop_core::Rect {
            x: 10.0,
            y: 10.0,
            width: 230.0,
            height: 410.0,
        }),
        state: agent_desktop_core::WindowState {
            is_focused: focused,
            visible: Some(true),
            minimized: Some(false),
            ..Default::default()
        },
    }
}

fn unplaced(id: &str) -> WindowInfo {
    let mut window = window(id, false);
    window.bounds = Some(agent_desktop_core::Rect {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    });
    window.state.visible = Some(false);
    window
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
    let picked = LaunchWindow::pick(vec![window("w-1", false), window("w-2", false)]);
    assert!(matches!(&picked, LaunchWindow::Several(ids) if ids == &["w-1", "w-2"]));
    let (shown, note) = picked.into_parts();
    assert!(shown.is_none());
    assert!(note.is_some_and(|note| note.contains("w-1, w-2")));
}

#[test]
fn no_window_is_absent_with_no_note() {
    let (shown, note) = LaunchWindow::pick(Vec::new()).into_parts();
    assert!(shown.is_none());
    assert!(note.is_none());
}

#[test]
fn a_window_not_yet_placed_on_screen_is_forming() {
    assert!(matches!(
        LaunchWindow::pick(vec![unplaced("w-1")]),
        LaunchWindow::Forming(forming) if forming.id == "w-1"
    ));
    let mut sizeless = unplaced("w-2");
    sizeless.bounds = None;
    assert!(matches!(
        LaunchWindow::pick(vec![sizeless]),
        LaunchWindow::Forming(_)
    ));
}

#[test]
fn an_off_screen_or_minimized_window_with_a_size_is_shown() {
    let mut other_space = window("w-1", false);
    other_space.state.visible = Some(false);
    assert!(matches!(
        LaunchWindow::pick(vec![other_space]),
        LaunchWindow::Shown(_)
    ));
    let mut minimized = unplaced("w-2");
    minimized.state.minimized = Some(true);
    assert!(matches!(
        LaunchWindow::pick(vec![minimized]),
        LaunchWindow::Shown(_)
    ));
}

#[test]
fn a_forming_window_is_still_reported() {
    let (shown, note) = LaunchWindow::Forming(unplaced("w-1")).into_parts();
    assert!(shown.is_some_and(|window| window.id == "w-1"));
    assert!(note.is_none());
}
