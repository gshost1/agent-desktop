use super::*;
use agent_desktop_core::{ProcessId, Rect, WindowState};

fn window(number: i64, visible: bool, minimized: Option<bool>, accessible: bool) -> WindowInfo {
    WindowInfo {
        id: format!("w-{number}"),
        title: String::new(),
        app: "Fixture".into(),
        pid: ProcessId::new(42),
        process_instance: Some("instance".into()),
        bounds: Some(Rect {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
        }),
        state: WindowState {
            is_focused: false,
            accessible,
            minimized,
            visible: Some(visible),
            on_other_space: None,
        },
    }
}

#[test]
fn an_unpublished_off_screen_window_on_another_space_is_marked() {
    let mut windows = vec![window(7, false, None, false)];
    mark_other_space_windows(&mut windows, |numbers| {
        assert_eq!(numbers, vec![7]);
        FxHashMap::from_iter([(7, true)])
    });
    assert_eq!(windows[0].state.on_other_space, Some(true));
}

#[test]
fn an_ordered_out_or_unknown_window_is_not_marked() {
    let mut windows = vec![window(7, false, None, false), window(8, false, None, false)];
    mark_other_space_windows(&mut windows, |_| FxHashMap::from_iter([(7, false)]));
    assert_eq!(windows[0].state.on_other_space, None);
    assert_eq!(windows[1].state.on_other_space, None);
}

#[test]
fn on_screen_minimized_published_and_sizeless_windows_make_no_space_read() {
    let mut sizeless = window(4, false, None, false);
    sizeless.bounds = Some(Rect {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    });
    let mut windows = vec![
        window(1, true, Some(false), true),
        window(2, false, Some(true), true),
        window(3, false, None, true),
        sizeless,
    ];
    mark_other_space_windows(&mut windows, |numbers| {
        panic!("no Space read expected, asked for {numbers:?}")
    });
    assert!(
        windows
            .iter()
            .all(|window| window.state.on_other_space.is_none())
    );
}

#[test]
fn space_reads_are_capped_per_inventory() {
    let mut windows = (1..=40)
        .map(|number| window(number, false, None, false))
        .collect::<Vec<_>>();
    mark_other_space_windows(&mut windows, |numbers| {
        assert_eq!(numbers.len(), MAX_SPACE_READS);
        numbers.into_iter().map(|number| (number, true)).collect()
    });
    let marked = windows
        .iter()
        .filter(|window| window.state.on_other_space == Some(true))
        .count();
    assert_eq!(marked, MAX_SPACE_READS);
}

#[test]
fn the_space_read_deadline_never_passes_the_inventory_deadline() {
    let deadline = Instant::now() + Duration::from_millis(5);
    assert!(space_read_deadline(deadline) <= deadline);
    let later = Instant::now() + Duration::from_secs(10);
    assert!(space_read_deadline(later) < later);
}
