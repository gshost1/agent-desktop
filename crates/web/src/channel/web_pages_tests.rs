use agent_desktop_core::{
    ObservationOps, OpenUrlRequest, ProcessId, SystemOps, WindowInfo, WindowState,
};
use serde_json::json;

use crate::channel::test_support::{
    INSTANCE, PID, PORT, ScriptedConnector, StubPlatform, adapter, attach, deadline, process,
    targets, window_of,
};
use crate::transport::scripted_transport::ScriptedTransport;

const BROWSER_AGENT: &str =
    "Mozilla/5.0 (Macintosh) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36";

fn browser() -> ScriptedTransport {
    ScriptedTransport::new().expect("Browser.getVersion", json!({ "userAgent": BROWSER_AGENT }))
}

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
    let mut transport = targets(browser(), &[("A", "about:blank", "")]);
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
    let mut transport = targets(browser(), &[]);
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
    assert_eq!(sent.lock().unwrap()[2]["params"]["background"], true);
}

#[test]
fn a_new_tab_opens_in_the_background() {
    let mut transport = browser().expect("Target.createTarget", json!({ "targetId": "N" }));
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
    assert_eq!(sent[1]["params"]["background"], true);
    assert_eq!(sent[1]["params"]["url"], "about:blank");
}

#[test]
fn an_app_that_embeds_the_runtime_is_never_navigated() {
    let transport = ScriptedTransport::new().expect(
        "Browser.getVersion",
        json!({ "userAgent": "Mozilla/5.0 (Macintosh) Chrome/130.0.0.0 Electron/33.4.0 Safari/537.36" }),
    );
    let sent = transport.sent_log();
    let web = adapter(
        StubPlatform::owning(PORT),
        ScriptedConnector::with(transport),
    );
    let window = window();
    let error = web.open_url(&request(Some(&window), false)).unwrap_err();
    assert_eq!(
        error.code,
        agent_desktop_core::ErrorCode::ActionNotSupported
    );
    assert_eq!(error.details.unwrap()["kind"], "cdp_not_a_browser");
    assert_eq!(sent.lock().unwrap().len(), 1);
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
fn surfaces_an_action_raised_never_reach_the_browser_endpoint() {
    let connector = ScriptedConnector {
        ports: vec![PORT],
        ..ScriptedConnector::default()
    };
    let opens = connector.opens.clone();
    let web = adapter(StubPlatform::owning(PORT), connector);
    let surfaces = web.list_raised_surfaces(process(), deadline()).unwrap();
    let kinds: Vec<&str> = surfaces
        .iter()
        .map(|surface| surface.kind.as_str())
        .collect();
    assert_eq!(kinds, vec!["window"]);
    assert!(opens.lock().unwrap().is_empty());
}
