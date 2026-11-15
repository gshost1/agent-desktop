use super::*;

#[test]
fn supported_surfaces_are_only_the_implemented_macos_contract() {
    assert_eq!(
        supported_surfaces_impl(),
        vec![
            SnapshotSurface::Window,
            SnapshotSurface::Focused,
            SnapshotSurface::Menu,
            SnapshotSurface::Menubar,
            SnapshotSurface::Sheet,
            SnapshotSurface::Popover,
            SnapshotSurface::Alert,
            SnapshotSurface::Dock,
            SnapshotSurface::MenuBarExtras,
        ]
    );
}

#[test]
fn map_surface_kind_covers_known_shapes() {
    assert_eq!(map_surface_kind("sheet"), Some(SnapshotSurface::Sheet));
    assert_eq!(map_surface_kind("popover"), Some(SnapshotSurface::Popover));
    assert_eq!(map_surface_kind("alert"), Some(SnapshotSurface::Alert));
    assert_eq!(map_surface_kind("menu"), Some(SnapshotSurface::Menu));
    assert_eq!(
        map_surface_kind("context_menu"),
        Some(SnapshotSurface::Menu)
    );
    assert_eq!(map_surface_kind("something_else"), None);
}

#[test]
fn matching_apps_filters_by_name_case_insensitively() {
    let filter = SignalFilter {
        app: Some("textedit".into()),
        process: None,
    };
    let apps = filter_apps(
        &filter,
        vec![
            AppInfo {
                name: "TextEdit".into(),
                pid: agent_desktop_core::ProcessId::new(42),
                bundle_id: None,
                process_instance: None,
                presentation: None,
            },
            AppInfo {
                name: "Finder".into(),
                pid: agent_desktop_core::ProcessId::new(7),
                bundle_id: None,
                process_instance: None,
                presentation: None,
            },
        ],
    );

    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].pid, 42);
}

#[test]
fn matching_apps_accepts_bundle_identifier() {
    let filter = SignalFilter {
        app: Some("com.example.editor".into()),
        process: None,
    };
    let apps = filter_apps(
        &filter,
        vec![AppInfo {
            name: "TextEdit".into(),
            pid: agent_desktop_core::ProcessId::new(42),
            bundle_id: Some("com.example.editor".into()),
            process_instance: None,
            presentation: None,
        }],
    );

    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].pid, 42);
}

#[test]
fn app_filter_with_no_constraints_preserves_the_complete_inventory() {
    let filter = SignalFilter::default();
    let apps = vec![AppInfo {
        name: "Finder".into(),
        pid: agent_desktop_core::ProcessId::new(7),
        bundle_id: None,
        process_instance: None,
        presentation: None,
    }];

    let filtered = filter_apps(&filter, apps);

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].name, "Finder");
    assert_eq!(filtered[0].pid, 7);
}

#[test]
fn surfaces_for_apps_is_empty_without_an_app_or_pid_filter() {
    let filter = SignalFilter::default();
    let apps = vec![AppInfo {
        name: "Finder".into(),
        pid: agent_desktop_core::ProcessId::new(1),
        bundle_id: None,
        process_instance: None,
        presentation: None,
    }];
    let deadline = Instant::now() + Duration::from_secs(5);
    let surfaces =
        surfaces_for_apps(&filter, &apps, deadline).expect("unscoped inventory succeeds");
    assert!(
        surfaces.signals.is_empty() && surfaces.complete,
        "unscoped waits must not walk every running app's AX tree for surfaces"
    );
}

#[test]
fn baseline_capture_rejects_an_expired_deadline() {
    let deadline = Instant::now() - Duration::from_millis(1);

    let error = capture_signal_baseline_impl(&SignalFilter::default(), deadline)
        .expect_err("an expired baseline capture must time out");

    assert_eq!(error.code.as_str(), "TIMEOUT");
}

fn surface_info(id: &str, kind: &str, unclassified: &[&str]) -> agent_desktop_core::SurfaceInfo {
    agent_desktop_core::SurfaceInfo {
        id: id.into(),
        kind: kind.into(),
        title: None,
        item_count: None,
        unclassified: unclassified.iter().map(|kind| (*kind).into()).collect(),
    }
}

fn probe_app() -> AppInfo {
    AppInfo {
        name: "Probe".into(),
        pid: agent_desktop_core::ProcessId::new(42),
        bundle_id: None,
        process_instance: Some("instance".into()),
        presentation: None,
    }
}

fn recorded(infos: Vec<agent_desktop_core::SurfaceInfo>) -> SurfaceCapture {
    let mut capture = SurfaceCapture {
        signals: Vec::new(),
        complete: true,
    };
    record_surfaces(&mut capture, &probe_app(), "instance", infos);
    capture
}

#[test]
fn a_fully_read_inventory_is_a_complete_surface_capture() {
    let capture = recorded(vec![surface_info("w-7", "sheet", &[])]);

    assert!(capture.complete);
    assert_eq!(capture.signals.len(), 1);
    assert_eq!(capture.signals[0].id, "w-7");
}

#[test]
fn an_unread_surface_kind_makes_the_capture_incomplete() {
    for unread in [
        surface_info("app/menubar", "menubar", &["menu"]),
        surface_info("app/windows", "window", &["sheet", "popover", "alert"]),
    ] {
        let capture = recorded(vec![surface_info("w-7", "sheet", &[]), unread]);

        assert!(!capture.complete);
        assert_eq!(capture.signals.len(), 1);
    }
}
