use super::*;

#[test]
fn merge_signal_baseline_adds_a_new_window_not_already_seen() {
    let seen = baseline_with_windows(vec![window("w-1", "Docs", "Finder", 100, true)]);
    let current = baseline_with_windows(vec![
        window("w-1", "Docs", "Finder", 100, true),
        window("w-2", "Untitled", "TextEdit", 200, false),
    ]);

    let merged = merge_signal_baseline(&seen, &current);

    assert_eq!(merged.windows.len(), 2);
}

#[test]
fn merge_signal_baseline_retains_an_entry_missing_from_current() {
    let seen = baseline_with_windows(vec![
        window("w-1", "Docs", "Finder", 100, true),
        window("w-2", "Untitled", "TextEdit", 200, false),
    ]);
    let current = baseline_with_windows(vec![window("w-1", "Docs", "Finder", 100, true)]);

    let merged = merge_signal_baseline(&seen, &current);

    assert_eq!(
        merged.windows.len(),
        2,
        "the running union must not drop an entry that disappeared this poll"
    );
}

#[test]
fn merge_signal_baseline_does_not_duplicate_an_already_seen_app() {
    let seen = SignalBaseline {
        windows: Vec::new(),
        apps: vec![app("Finder", 1)],
        surfaces: Vec::new(),
        completeness: crate::SignalCompleteness::complete(),
    };
    let current = SignalBaseline {
        windows: Vec::new(),
        apps: vec![app("Finder", 1)],
        surfaces: Vec::new(),
        completeness: crate::SignalCompleteness::complete(),
    };

    let merged = merge_signal_baseline(&seen, &current);

    assert_eq!(merged.apps.len(), 1);
}

#[test]
fn merge_signal_baseline_round_trip_detects_appear_then_disappear() {
    let seen = baseline_with_windows(Vec::new());
    let appeared = baseline_with_windows(vec![window("w-9", "Untitled", "TextEdit", 200, false)]);
    let seen = merge_signal_baseline(&seen, &appeared);
    let disappeared = baseline_with_windows(Vec::new());

    let events = diff_signals(&seen, &disappeared);

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, EventKind::WindowClosed);
    assert_eq!(events[0].window_id.as_deref(), Some("w-9"));
}

fn menu_surface(id: &str) -> SurfaceSignal {
    SurfaceSignal {
        kind: SnapshotSurface::Menu,
        app: "TextEdit".into(),
        pid: crate::ProcessId::new(200),
        process_instance: "instance-200".into(),
        id: id.into(),
        title: None,
    }
}

fn with_surfaces(windows: Vec<WindowInfo>, surfaces: Vec<SurfaceSignal>) -> SignalBaseline {
    SignalBaseline {
        surfaces,
        ..baseline_with_windows(windows)
    }
}

#[test]
fn shrink_signal_floor_drops_a_surface_that_left_and_keeps_one_still_present() {
    let floor = with_surfaces(
        Vec::new(),
        vec![menu_surface("menu-7"), menu_surface("menu-8")],
    );
    let current = with_surfaces(Vec::new(), vec![menu_surface("menu-8")]);

    let mut shrunk = floor.clone();
    shrink_signal_floor(&mut shrunk, &current);

    assert_eq!(shrunk.surfaces, vec![menu_surface("menu-8")]);
    assert!(
        diff_signals(&shrunk, &floor)
            .iter()
            .any(|event| matches!(event.kind, EventKind::SurfaceAppeared { .. }))
    );
}

#[test]
fn shrink_signal_floor_never_adds_a_surface_that_only_current_shows() {
    let mut floor = with_surfaces(Vec::new(), Vec::new());
    let current = with_surfaces(Vec::new(), vec![menu_surface("menu-7")]);

    shrink_signal_floor(&mut floor, &current);

    assert!(floor.surfaces.is_empty());
}

#[test]
fn shrink_signal_floor_keeps_every_surface_when_the_capture_is_incomplete() {
    let mut floor = with_surfaces(Vec::new(), vec![menu_surface("menu-7")]);
    let mut current = with_surfaces(Vec::new(), Vec::new());
    current.completeness.surfaces = false;

    shrink_signal_floor(&mut floor, &current);

    assert_eq!(floor.surfaces, vec![menu_surface("menu-7")]);
}

#[test]
fn shrink_signal_floor_leaves_windows_and_completeness_at_their_baseline() {
    let floor = with_surfaces(
        vec![window("w-1", "Docs", "Finder", 100, true)],
        vec![menu_surface("menu-7")],
    );
    let mut current = with_surfaces(Vec::new(), Vec::new());
    current.completeness.windows = false;
    let mut shrunk = floor.clone();

    shrink_signal_floor(&mut shrunk, &current);

    assert_eq!(shrunk.windows.len(), 1);
    assert_eq!(shrunk.completeness, floor.completeness);
}

#[test]
fn shrink_signal_floor_takes_the_first_complete_surface_list_when_it_started_incomplete() {
    let mut floor = with_surfaces(Vec::new(), Vec::new());
    floor.completeness.surfaces = false;
    let current = with_surfaces(Vec::new(), vec![menu_surface("menu-7")]);

    shrink_signal_floor(&mut floor, &current);

    assert_eq!(floor.surfaces, vec![menu_surface("menu-7")]);
    assert!(floor.completeness.surfaces);
}
