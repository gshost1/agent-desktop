use agent_desktop_core::{
    AppInfo, ErrorCode, ObservationOps, ObservationRequest, ObservationRoot, ProcessId,
    ProcessIdentity, SnapshotSurface, SystemOps, TreeOptions, WindowInfo,
};

use crate::channel::test_support::{
    INSTANCE, PID, PORT, ScriptedConnector, StubPlatform, adapter, attach, deadline, page_snapshot,
    targets, window_of,
};
use crate::transport::scripted_transport::ScriptedTransport;

fn web_request() -> ObservationRequest {
    let options = TreeOptions {
        surface: SnapshotSurface::Web,
        ..TreeOptions::default()
    };
    ObservationRequest::snapshot(&options, deadline())
}

fn browser_process() -> WindowInfo {
    WindowInfo::for_process(&AppInfo {
        name: "Browser".into(),
        pid: ProcessId::new(PID),
        process_instance: Some(INSTANCE.into()),
        bundle_id: None,
        presentation: None,
    })
}

#[test]
fn a_browser_owned_through_its_process_shows_its_page_over_cdp() {
    let transport = targets(
        ScriptedTransport::new(),
        &[("A", "https://docs.test/", "Docs")],
    );
    let transport = page_snapshot(attach(window_of(transport, 1, 0.0), "A"));
    let web = adapter(
        StubPlatform::owning(PORT),
        ScriptedConnector::with(transport),
    );
    let owner = browser_process();
    let observed = web
        .observe_tree(ObservationRoot::Window(&owner), &web_request())
        .unwrap();
    let channel = observed.channel().cloned().unwrap();
    assert_eq!(channel.channel, "cdp");
    assert_eq!(channel.url.as_deref(), Some("https://docs.test/"));
    assert!(web.inner().surfaces_seen.lock().unwrap().is_empty());
}

#[test]
fn a_browser_owned_through_its_process_is_not_read_through_accessibility() {
    let web = adapter(StubPlatform::default(), ScriptedConnector::default());
    let owner = browser_process();
    let error = web
        .observe_tree(ObservationRoot::Window(&owner), &web_request())
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::WindowNotFound);
    assert!(error.suggestion.unwrap().contains("--activate"));
    assert!(web.inner().surfaces_seen.lock().unwrap().is_empty());
}

#[test]
fn only_an_endpoint_launch_recorded_counts_as_recorded() {
    let process = ProcessIdentity::new(PID, INSTANCE);
    let recorded = adapter(
        StubPlatform::default(),
        ScriptedConnector::with(ScriptedTransport::new()),
    );
    assert!(recorded.has_recorded_cdp_endpoint(process.clone()));
    let unrecorded = adapter(StubPlatform::default(), ScriptedConnector::default());
    assert!(!unrecorded.has_recorded_cdp_endpoint(process));
}
