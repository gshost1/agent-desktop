use agent_desktop_core::{Deadline, ErrorCode};
use serde_json::json;

use super::{navigate, wait_for_load};
use crate::transport::scripted_transport::ScriptedTransport;
use crate::{CdpConnection, PageSession};

fn session() -> PageSession {
    PageSession {
        target_id: "T".into(),
        session_id: "S".into(),
    }
}

fn enabled() -> ScriptedTransport {
    ScriptedTransport::new()
        .expect("Page.enable", json!({}))
        .expect("Page.setLifecycleEventsEnabled", json!({}))
}

#[test]
fn navigation_waits_for_the_matching_lifecycle_load() {
    let transport = enabled().expect_with_events(
        "Page.navigate",
        vec![
            json!({ "method": "Page.lifecycleEvent", "sessionId": "S",
                "params": { "name": "load", "frameId": "F", "loaderId": "L1" } }),
            json!({ "method": "Page.lifecycleEvent", "sessionId": "OTHER",
                "params": { "name": "load", "frameId": "F", "loaderId": "L2" } }),
            json!({ "method": "Page.lifecycleEvent", "sessionId": "S",
                "params": { "name": "load", "frameId": "F", "loaderId": "L2" } }),
        ],
        json!({ "result": { "frameId": "F", "loaderId": "L2" } }),
    );
    let mut connection = CdpConnection::new(transport.boxed());

    navigate(
        &mut connection,
        &session(),
        "https://a.test/",
        Deadline::after(500).unwrap(),
    )
    .unwrap();

    assert_eq!(connection.take_events().len(), 2);
}

#[test]
fn a_navigation_error_text_fails_without_waiting() {
    let transport = enabled().expect(
        "Page.navigate",
        json!({ "frameId": "F", "loaderId": "L", "errorText": "net::ERR_NAME_NOT_RESOLVED" }),
    );
    let mut connection = CdpConnection::new(transport.boxed());

    let error = navigate(
        &mut connection,
        &session(),
        "https://nope.test/",
        Deadline::after(500).unwrap(),
    )
    .unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.platform_detail.as_deref(),
        Some("net::ERR_NAME_NOT_RESOLVED")
    );
}

#[test]
fn a_missing_load_is_a_bounded_timeout() {
    let transport = enabled().expect("Page.navigate", json!({ "frameId": "F", "loaderId": "L" }));
    let mut connection = CdpConnection::new(transport.boxed());

    let error = navigate(
        &mut connection,
        &session(),
        "https://slow.test/",
        Deadline::after(50).unwrap(),
    )
    .unwrap_err();

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.details.as_ref().unwrap()["kind"], "cdp_load_timeout");
}

#[test]
fn a_loaded_document_needs_no_wait() {
    let transport = enabled().expect(
        "Runtime.evaluate",
        json!({ "result": { "type": "string", "value": "complete" } }),
    );
    let mut connection = CdpConnection::new(transport.boxed());

    wait_for_load(&mut connection, &session(), Deadline::after(500).unwrap()).unwrap();
}

#[test]
fn a_loading_document_waits_for_its_load_event() {
    let transport = enabled()
        .expect(
            "Runtime.evaluate",
            json!({ "result": { "type": "string", "value": "loading" } }),
        )
        .push_inbound(json!({ "method": "Page.loadEventFired", "sessionId": "S", "params": {} }));
    let mut connection = CdpConnection::new(transport.boxed());

    wait_for_load(&mut connection, &session(), Deadline::after(500).unwrap()).unwrap();
}
