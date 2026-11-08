use agent_desktop_core::{
    AppInfo, ErrorCode, ObservationOps, ObservationRequest, ObservationRoot, ProcessId, Rect,
    SnapshotSurface, TreeOptions, WindowInfo, WindowState,
};

use crate::channel::test_support::{
    INSTANCE, PID, PORT, ScriptedConnector, StubPlatform, adapter, attach, deadline, page_snapshot,
    targets, window_of,
};
use crate::transport::scripted_transport::ScriptedTransport;

fn request(surface: SnapshotSurface) -> ObservationRequest {
    let options = TreeOptions {
        surface,
        ..TreeOptions::default()
    };
    ObservationRequest::snapshot(&options, deadline())
}

fn browser_window() -> WindowInfo {
    WindowInfo {
        id: "w1".into(),
        title: "Docs".into(),
        app: "Browser".into(),
        pid: ProcessId::new(PID),
        process_instance: Some(INSTANCE.into()),
        bounds: Some(Rect {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
        }),
        state: WindowState::default(),
    }
}

#[test]
fn the_web_surface_reads_the_shown_page_over_cdp() {
    let mut transport = targets(
        ScriptedTransport::new(),
        &[("A", "https://docs.test/", "Docs")],
    );
    transport = page_snapshot(attach(window_of(transport, 1, 0.0), "A"));
    let platform = StubPlatform::owning(PORT);
    let web = adapter(platform, ScriptedConnector::with(transport));
    let window = browser_window();
    let observed = web
        .observe_tree(
            ObservationRoot::Window(&window),
            &request(SnapshotSurface::Web),
        )
        .unwrap();
    let channel = observed.channel().cloned().unwrap();
    assert_eq!(channel.channel, "cdp");
    assert_eq!(channel.url.as_deref(), Some("https://docs.test/"));
    assert_eq!(channel.title.as_deref(), Some("Docs"));
    let tree = observed.into_accessibility_tree().unwrap();
    assert_eq!(tree.children.len(), 2);
    assert_eq!(
        tree.children[0].identity.retained_object.as_deref(),
        Some("cdp:A:MAIN:LOAD:20")
    );
    assert!(web.inner().recorded().is_empty());
}

#[test]
fn without_an_endpoint_the_window_is_read_through_accessibility() {
    let web = adapter(StubPlatform::default(), ScriptedConnector::default());
    let window = browser_window();
    let observed = web
        .observe_tree(
            ObservationRoot::Window(&window),
            &request(SnapshotSurface::Web),
        )
        .unwrap();
    let channel = observed.channel().cloned().unwrap();
    assert_eq!(channel.channel, "ax");
    assert!(
        channel
            .suggestion
            .unwrap()
            .contains("launch \"Browser\" --cdp")
    );
    assert_eq!(
        *web.inner().surfaces_seen.lock().unwrap(),
        vec![SnapshotSurface::Window]
    );
}

#[test]
fn other_surfaces_go_to_the_platform_unchanged_and_carry_no_channel() {
    let web = adapter(StubPlatform::owning(PORT), ScriptedConnector::default());
    let window = browser_window();
    let observed = web
        .observe_tree(
            ObservationRoot::Window(&window),
            &request(SnapshotSurface::Menu),
        )
        .unwrap();
    assert!(observed.channel().is_none());
    assert_eq!(
        *web.inner().surfaces_seen.lock().unwrap(),
        vec![SnapshotSurface::Menu]
    );
}

#[test]
fn an_application_root_has_no_single_page() {
    let web = adapter(StubPlatform::default(), ScriptedConnector::default());
    let app = AppInfo {
        name: "Browser".into(),
        pid: ProcessId::new(PID),
        process_instance: Some(INSTANCE.into()),
        bundle_id: None,
        presentation: None,
    };
    let root = ObservationRoot::Application {
        app: &app,
        surface: SnapshotSurface::Web,
    };
    let error = web
        .observe_tree(root, &request(SnapshotSurface::Web))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidArgs);
}
