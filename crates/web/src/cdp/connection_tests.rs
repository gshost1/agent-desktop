use agent_desktop_core::{Deadline, ErrorCode};
use serde_json::json;

use super::CdpConnection;
use crate::cdp::protocol_error::is_protocol_error;
use crate::transport::scripted_transport::ScriptedTransport;

fn deadline() -> Deadline {
    Deadline::after(1_000).unwrap()
}

#[test]
fn ids_are_monotonic_and_session_is_flattened_into_the_request() {
    let transport = ScriptedTransport::new()
        .expect("Browser.getVersion", json!({ "product": "Chrome/144" }))
        .expect("DOM.focus", json!({}));
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());

    let version = connection
        .call("Browser.getVersion", json!({}), None, deadline())
        .unwrap();
    connection
        .call(
            "DOM.focus",
            json!({ "backendNodeId": 4 }),
            Some("S1"),
            deadline(),
        )
        .unwrap();

    assert_eq!(version["product"], "Chrome/144");
    let sent = sent.lock().unwrap();
    assert_eq!(sent[0]["id"], 1);
    assert!(sent[0].get("sessionId").is_none());
    assert_eq!(sent[1]["id"], 2);
    assert_eq!(sent[1]["sessionId"], "S1");
    assert_eq!(sent[1]["params"]["backendNodeId"], 4);
}

#[test]
fn events_received_while_waiting_are_buffered_in_order() {
    let transport = ScriptedTransport::new().expect_with_events(
        "Page.navigate",
        vec![
            json!({ "method": "Page.frameStartedLoading", "params": { "frameId": "F" }, "sessionId": "S" }),
            json!({ "method": "Page.loadEventFired", "params": {}, "sessionId": "S" }),
        ],
        json!({ "result": { "frameId": "F" } }),
    );
    let mut connection = CdpConnection::new(transport.boxed());

    connection
        .call(
            "Page.navigate",
            json!({ "url": "about:blank" }),
            Some("S"),
            deadline(),
        )
        .unwrap();
    let load = connection
        .wait_for_event(|event| event.method == "Page.loadEventFired", deadline())
        .unwrap();

    assert_eq!(load.session_id.as_deref(), Some("S"));
    let rest = connection.take_events();
    assert_eq!(rest.len(), 1);
    assert_eq!(rest[0].method, "Page.frameStartedLoading");
}

#[test]
fn stale_replies_from_an_abandoned_request_are_skipped() {
    let transport = ScriptedTransport::new()
        .push_inbound(json!({ "id": 99, "result": { "stale": true } }))
        .expect("Target.getTargets", json!({ "targetInfos": [] }));
    let mut connection = CdpConnection::new(transport.boxed());

    let result = connection
        .call("Target.getTargets", json!({}), None, deadline())
        .unwrap();

    assert_eq!(result, json!({ "targetInfos": [] }));
}

#[test]
fn cdp_error_maps_to_adapter_error_with_platform_detail() {
    let transport = ScriptedTransport::new()
        .expect_error("DOM.resolveNode", -32000, "No node with given id found")
        .expect_error("Nope.method", -32601, "'Nope.method' wasn't found");
    let mut connection = CdpConnection::new(transport.boxed());

    let error = connection
        .call("DOM.resolveNode", json!({}), Some("S"), deadline())
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.platform_detail.as_deref(),
        Some("No node with given id found")
    );
    assert!(is_protocol_error(&error));
    assert_eq!(error.details.as_ref().unwrap()["cdp_code"], -32000);

    let missing = connection
        .call("Nope.method", json!({}), None, deadline())
        .unwrap_err();
    assert_eq!(missing.code, ErrorCode::ActionNotSupported);
}

#[test]
fn the_event_buffer_is_bounded_and_counts_drops() {
    let events = (0..1030)
        .map(|index| json!({ "method": "Network.dataReceived", "params": { "n": index } }))
        .collect();
    let transport = ScriptedTransport::new().expect_with_events(
        "Runtime.runIfWaitingForDebugger",
        events,
        json!({ "result": {} }),
    );
    let mut connection = CdpConnection::new(transport.boxed());

    connection
        .call(
            "Runtime.runIfWaitingForDebugger",
            json!({}),
            None,
            deadline(),
        )
        .unwrap();

    assert_eq!(connection.dropped_event_count(), 6);
    let kept = connection.take_events();
    assert_eq!(kept.len(), 1024);
    assert_eq!(kept[0].params["n"], 6);
}

#[test]
fn a_silent_transport_surfaces_a_timeout() {
    let mut connection = CdpConnection::new(ScriptedTransport::new().boxed());

    let error = connection.wait_for_event(|_| true, deadline()).unwrap_err();

    assert_eq!(error.code, ErrorCode::Timeout);
}
