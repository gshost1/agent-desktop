use agent_desktop_core::{ErrorCode, ObservationOps, SnapshotSurface};
use serde_json::json;

use super::live_element;
use crate::channel::test_support::{
    PORT, ScriptedConnector, StubPlatform, adapter, attach, deadline, element_read, node,
    node_read, process, web_entry,
};
use crate::channel::web_handle::WebHandle;
use crate::transport::scripted_transport::ScriptedTransport;
use crate::{ElementReading, ElementToken};

fn web_with(transport: ScriptedTransport) -> crate::WebChannelAdapter<StubPlatform> {
    adapter(
        StubPlatform::owning(PORT),
        ScriptedConnector::with(transport),
    )
}

#[test]
fn a_live_node_with_the_captured_role_and_name_resolves_to_a_web_handle() {
    let web = web_with(node_read(
        attach(ScriptedTransport::new(), "T"),
        7,
        "button",
        "Go",
    ));
    let handle = web
        .resolve_element_strict(&web_entry("cdp:T:7", "button", "Go"), deadline())
        .unwrap();
    let web_handle = WebHandle::of(&handle).unwrap();
    assert_eq!(web_handle.process, process());
    assert_eq!(web_handle.token.backend_node_id, 7);
    assert!(web.inner().recorded().is_empty());
}

#[test]
fn a_node_the_page_dropped_is_stale() {
    let transport = attach(ScriptedTransport::new(), "T").expect_error(
        "DOM.resolveNode",
        -32000,
        "No node with given id found",
    );
    let error = web_with(transport)
        .resolve_element_strict(&web_entry("cdp:T:7", "button", "Go"), deadline())
        .err()
        .unwrap();
    assert_eq!(error.code, ErrorCode::StaleRef);
}

#[test]
fn a_node_whose_role_or_name_changed_is_stale() {
    for (role, name) in [("link", "Go"), ("button", "Stop")] {
        let web = web_with(node_read(
            attach(ScriptedTransport::new(), "T"),
            7,
            role,
            name,
        ));
        let error = web
            .resolve_element_strict(&web_entry("cdp:T:7", "button", "Go"), deadline())
            .err()
            .unwrap();
        assert_eq!(error.code, ErrorCode::StaleRef, "{role} {name}");
        assert_eq!(error.details.unwrap()["kind"], "cdp_node_drift");
    }
}

#[test]
fn a_closed_page_is_stale() {
    let transport = ScriptedTransport::new().expect_error(
        "Target.attachToTarget",
        -32602,
        "No target with given id found",
    );
    let error = web_with(transport)
        .resolve_element_strict(&web_entry("cdp:T:7", "button", "Go"), deadline())
        .err()
        .unwrap();
    assert_eq!(error.code, ErrorCode::StaleRef);
}

#[test]
fn a_native_ref_on_the_web_surface_resolves_through_the_platform_as_a_window_ref() {
    let web = adapter(StubPlatform::default(), ScriptedConnector::default());
    web.resolve_element_strict(&web_entry("axtok1:abc", "button", "Go"), deadline())
        .unwrap();
    assert_eq!(web.inner().recorded(), vec!["resolve_element_strict"]);
    assert_eq!(
        *web.inner().surfaces_seen.lock().unwrap(),
        vec![SnapshotSurface::Window]
    );
}

#[test]
fn the_live_read_reports_dom_state_without_bounds() {
    let mut transport = node_read(
        attach(ScriptedTransport::new(), "T"),
        7,
        "checkbox",
        "Agree",
    );
    transport = element_read(transport, json!("on"), json!(true));
    let web = web_with(transport);
    let handle = agent_desktop_core::NativeHandle::new(WebHandle {
        process: process(),
        token: ElementToken::parse("cdp:T:7").unwrap(),
    });
    let live = web.get_live_element(&handle, deadline()).unwrap();
    assert!(live.bounds.is_none());
    assert_eq!(live.state.value.as_deref(), Some("on"));
    assert!(live.state.states.iter().any(|state| state == "checked"));
    assert_eq!(live.identity.identifiers.retained_object(), Some("cdp:T:7"));
    assert!(web.is_nonspatial_target(&handle));
    assert!(web.pointer_position_refusal(&handle, deadline()).is_some());
    assert!(web.inner().recorded().is_empty());
}

#[test]
fn dom_checked_state_wins_over_the_accessibility_state() {
    let mut checked = node("checkbox", "Agree", Some("cdp:T:7"));
    checked.presentation.states = vec!["checked".into()];
    let reading = ElementReading {
        checked: Some(false),
        ..ElementReading::default()
    };
    let live = live_element(checked, reading, &ElementToken::parse("cdp:T:7").unwrap());
    assert!(!live.state.states.iter().any(|state| state == "checked"));
    assert!(
        live.available_actions
            .iter()
            .any(|action| action == "PressKey")
    );
    assert_eq!(live.state.offscreen, Some(false));
    assert_eq!(live.state.hidden, Some(false));
}
