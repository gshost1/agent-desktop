use agent_desktop_core::{
    ObservationOps, OpenUrlRequest, ProcessId, SystemOps, WindowInfo, WindowState,
};
use serde_json::json;

use crate::channel::test_support::{
    INSTANCE, PID, PORT, ScriptedConnector, StubPlatform, adapter, attach, deadline, process,
    targets, window_of,
};
use crate::transport::scripted_transport::ScriptedTransport;

fn navigated(transport: ScriptedTransport) -> ScriptedTransport {
    transport
        .expect("Page.enable", json!({}))
        .expect("Page.setLifecycleEventsEnabled", json!({}))
        .expect("Page.navigate", json!({ "frameId": "F" }))
}

fn window() -> WindowInfo {
    WindowInfo {
        id: "w1".into(),
        title: "Docs".into(),
        app: "Browser".into(),
        pid: ProcessId::new(PID),
        process_instance: Some(INSTANCE.into()),
        bounds: None,
        state: WindowState::default(),
    }
}

fn request<'a>(window: Option<&'a WindowInfo>, new_tab: bool) -> OpenUrlRequest<'a> {
    OpenUrlRequest {
        process: process(),
        window,
        url: "https://example.test/",
        new_tab,
        deadline: deadline(),
    }
}

#[test]
fn the_url_loads_in_the_tab_the_window_shows() {
    let mut transport = targets(ScriptedTransport::new(), &[("A", "about:blank", "")]);
    transport = navigated(attach(window_of(transport, 1, 0.0), "A"));
    transport = targets(transport, &[("A", "https://example.test/", "Example")]);
    let web = adapter(
        StubPlatform::owning(PORT),
        ScriptedConnector::with(transport),
    );
    let window = window();
    let page = web.open_url(&request(Some(&window), false)).unwrap();
    assert_eq!(page.target_id, "A");
    assert_eq!(page.title, "Example");
    assert_eq!(page.channel, "cdp");
    assert_eq!(page.opened, "current_tab");
    assert!(web.inner().recorded().is_empty());
}

#[test]
fn a_browser_with_no_page_gets_a_background_tab_instead_of_an_error() {
    let mut transport = targets(ScriptedTransport::new(), &[]);
    transport = transport.expect("Target.createTarget", json!({ "targetId": "N" }));
    transport = navigated(attach(transport, "N"));
    transport = targets(transport, &[("N", "https://example.test/", "Example")]);
    let sent = transport.sent_log();
    let web = adapter(
        StubPlatform::owning(PORT),
        ScriptedConnector::with(transport),
    );
    let window = window();
    let page = web.open_url(&request(Some(&window), false)).unwrap();
    assert_eq!(page.target_id, "N");
    assert_eq!(page.opened, "new_tab");
    assert_eq!(sent.lock().unwrap()[1]["params"]["background"], true);
}

#[test]
fn a_new_tab_opens_in_the_background() {
    let mut transport =
        ScriptedTransport::new().expect("Target.createTarget", json!({ "targetId": "N" }));
    transport = navigated(attach(transport, "N"));
    transport = targets(transport, &[("N", "https://example.test/", "Example")]);
    let sent = transport.sent_log();
    let web = adapter(
        StubPlatform::owning(PORT),
        ScriptedConnector::with(transport),
    );
    let page = web.open_url(&request(None, true)).unwrap();
    assert_eq!(page.target_id, "N");
    assert_eq!(page.opened, "new_tab");
    let sent = sent.lock().unwrap();
    assert_eq!(sent[0]["params"]["background"], true);
    assert_eq!(sent[0]["params"]["url"], "about:blank");
}

#[test]
fn without_an_endpoint_the_platform_answers() {
    let web = adapter(StubPlatform::default(), ScriptedConnector::default());
    let error = web.open_url(&request(None, true)).unwrap_err();
    assert_eq!(
        error.code,
        agent_desktop_core::ErrorCode::PlatformNotSupported
    );
    assert_eq!(web.inner().recorded(), vec!["open_url"]);
}

#[test]
fn surfaces_list_one_web_surface_per_page_without_prompting() {
    let transport = targets(
        ScriptedTransport::new(),
        &[("A", "https://a.test/", "A"), ("B", "https://b.test/", "")],
    );
    let connector = ScriptedConnector::with(transport);
    let opens = connector.opens.clone();
    let web = adapter(StubPlatform::owning(PORT), connector);
    let surfaces = web.list_surfaces(process(), deadline()).unwrap();
    let kinds: Vec<(&str, &str)> = surfaces
        .iter()
        .map(|surface| (surface.kind.as_str(), surface.id.as_str()))
        .collect();
    assert_eq!(kinds, vec![("window", "w1"), ("web", "A"), ("web", "B")]);
    assert_eq!(surfaces[2].title.as_deref(), Some("https://b.test/"));
    assert_eq!(*opens.lock().unwrap(), vec![(PORT, false)]);
}

#[test]
fn an_unreachable_browser_has_only_its_platform_surfaces() {
    let web = adapter(StubPlatform::default(), ScriptedConnector::default());
    assert_eq!(web.list_surfaces(process(), deadline()).unwrap().len(), 1);
}

#[test]
fn a_url_the_browser_saves_as_a_download_is_reported_as_one() {
    let mut transport = targets(ScriptedTransport::new(), &[("A", "about:blank", "")]);
    transport = attach(window_of(transport, 1, 0.0), "A")
        .expect("Page.enable", json!({}))
        .expect("Page.setLifecycleEventsEnabled", json!({}))
        .expect_with_events(
            "Page.navigate",
            vec![json!({
                "method": "Browser.downloadWillBegin",
                "params": { "guid": "g1", "url": "https://example.test/", "suggestedFilename": "report.csv" }
            })],
            json!({ "result": { "frameId": "F", "errorText": "net::ERR_ABORTED" } }),
        );
    let web = adapter(
        StubPlatform::owning(PORT),
        ScriptedConnector::with(transport),
    );
    let window = window();
    let page = web.open_url(&request(Some(&window), false)).unwrap();
    assert_eq!(page.opened, "download");
    assert_eq!(page.target_id, "A");
}

#[test]
fn a_failed_navigation_without_a_download_stays_an_error() {
    let mut transport = targets(ScriptedTransport::new(), &[("A", "about:blank", "")]);
    transport = attach(window_of(transport, 1, 0.0), "A")
        .expect("Page.enable", json!({}))
        .expect("Page.setLifecycleEventsEnabled", json!({}))
        .expect(
            "Page.navigate",
            json!({ "frameId": "F", "errorText": "net::ERR_NAME_NOT_RESOLVED" }),
        );
    let web = adapter(
        StubPlatform::owning(PORT),
        ScriptedConnector::with(transport),
    );
    let window = window();
    let error = web.open_url(&request(Some(&window), false)).err().unwrap();
    assert_eq!(error.details.unwrap()["kind"], "cdp_navigation_failed");
}
