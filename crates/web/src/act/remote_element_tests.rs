use agent_desktop_core::{Deadline, ErrorCode};
use serde_json::json;

use super::{call_on_element, ensure_live};
use crate::act::element_call::ElementCall;
use crate::transport::scripted_transport::ScriptedTransport;
use crate::{CdpConnection, PageSession};

fn session() -> PageSession {
    PageSession {
        target_id: "T".into(),
        session_id: "S".into(),
    }
}

fn deadline() -> Deadline {
    Deadline::after(1_000).unwrap()
}

#[test]
fn a_node_the_page_dropped_is_a_stale_ref() {
    let transport = ScriptedTransport::new().expect_error(
        "DOM.resolveNode",
        -32000,
        "No node with given id found",
    );
    let mut connection = CdpConnection::new(transport.boxed());

    let error = ensure_live(&mut connection, &session(), 77, deadline()).unwrap_err();

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(
        error.platform_detail.as_deref(),
        Some("No node with given id found")
    );
    assert_eq!(error.details.as_ref().unwrap()["backend_node_id"], 77);
}

#[test]
fn a_transport_timeout_is_not_mistaken_for_staleness() {
    let transport = ScriptedTransport::new().expect_silence("DOM.resolveNode");
    let mut connection = CdpConnection::new(transport.boxed());

    let error = ensure_live(
        &mut connection,
        &session(),
        77,
        Deadline::after(30).unwrap(),
    )
    .unwrap_err();

    assert_eq!(error.code, ErrorCode::Timeout);
}

#[test]
fn element_calls_bind_the_resolved_object_and_release_it() {
    let transport = ScriptedTransport::new()
        .expect("DOM.resolveNode", json!({ "object": { "objectId": "O1" } }))
        .expect(
            "Runtime.callFunctionOn",
            json!({ "result": { "type": "string", "value": "ok" } }),
        )
        .expect("Runtime.releaseObject", json!({}));
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());

    let value = call_on_element(
        &mut connection,
        &session(),
        5,
        ElementCall {
            function: "function(x) { return x; }",
            arguments: vec![json!("ok")],
        },
        deadline(),
    )
    .unwrap();

    assert_eq!(value, json!("ok"));
    let sent = sent.lock().unwrap();
    assert_eq!(sent[0]["params"]["backendNodeId"], 5);
    assert_eq!(sent[1]["params"]["objectId"], "O1");
    assert_eq!(sent[1]["params"]["arguments"], json!([{ "value": "ok" }]));
    assert_eq!(sent[1]["params"]["returnByValue"], true);
    assert_eq!(sent[2]["params"]["objectId"], "O1");
    assert!(
        sent.iter()
            .all(|request| request["method"] != "Runtime.enable")
    );
}

#[test]
fn a_throwing_script_is_an_action_failure() {
    let transport = ScriptedTransport::new()
        .expect("DOM.resolveNode", json!({ "object": { "objectId": "O1" } }))
        .expect(
            "Runtime.callFunctionOn",
            json!({ "result": {}, "exceptionDetails": { "text": "Uncaught", "exception": { "description": "TypeError: boom" } } }),
        )
        .expect("Runtime.releaseObject", json!({}));
    let mut connection = CdpConnection::new(transport.boxed());

    let error = call_on_element(
        &mut connection,
        &session(),
        5,
        ElementCall {
            function: "function() { throw new TypeError('boom'); }",
            arguments: Vec::new(),
        },
        deadline(),
    )
    .unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.platform_detail.as_deref(), Some("TypeError: boom"));
}
