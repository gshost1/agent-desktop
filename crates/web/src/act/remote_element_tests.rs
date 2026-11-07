use agent_desktop_core::{Deadline, ErrorCode};
use serde_json::json;

use super::call_on_element;
use crate::CdpConnection;
use crate::act::act_fixture::{deadline, proven, resolved, session, token};
use crate::act::element_call::ElementCall;
use crate::transport::scripted_transport::ScriptedTransport;

fn echo() -> ElementCall {
    ElementCall {
        function: "function(x) { return x; }",
        arguments: vec![json!("ok")],
    }
}

#[test]
fn a_node_the_page_dropped_is_a_stale_ref() {
    let transport = proven(ScriptedTransport::new()).expect_error(
        "DOM.resolveNode",
        -32000,
        "No node with given id found",
    );
    let mut connection = CdpConnection::new(transport.boxed());

    let error =
        call_on_element(&mut connection, &session(), &token(77), echo(), deadline()).unwrap_err();

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(
        error.platform_detail.as_deref(),
        Some("No node with given id found")
    );
    assert_eq!(error.details.as_ref().unwrap()["backend_node_id"], 77);
}

#[test]
fn a_lost_session_is_target_gone_not_a_stale_ref() {
    let transport = proven(ScriptedTransport::new()).expect_error(
        "DOM.resolveNode",
        -32001,
        "Session with given id not found.",
    );
    let mut connection = CdpConnection::new(transport.boxed());

    let error =
        call_on_element(&mut connection, &session(), &token(77), echo(), deadline()).unwrap_err();

    assert_eq!(error.code, ErrorCode::WindowNotFound);
    assert_eq!(error.details.as_ref().unwrap()["kind"], "cdp_target_gone");
}

#[test]
fn any_other_refusal_to_resolve_is_not_a_stale_ref() {
    let transport = proven(ScriptedTransport::new()).expect_error(
        "DOM.resolveNode",
        -32000,
        "Cannot find context with specified id",
    );
    let mut connection = CdpConnection::new(transport.boxed());

    let error =
        call_on_element(&mut connection, &session(), &token(77), echo(), deadline()).unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionFailed);
}

#[test]
fn a_replaced_document_is_stale_before_the_node_is_touched() {
    let transport = ScriptedTransport::new().expect(
        "Page.getFrameTree",
        json!({ "frameTree": { "frame": { "id": "F", "loaderId": "NEWER" } } }),
    );
    let mut connection = CdpConnection::new(transport.boxed());

    let error =
        call_on_element(&mut connection, &session(), &token(77), echo(), deadline()).unwrap_err();

    assert_eq!(error.code, ErrorCode::StaleRef);
    let details = error.details.unwrap();
    assert_eq!(details["kind"], "cdp_document_replaced");
    assert_eq!(details["current_loader_id"], "NEWER");
}

#[test]
fn a_frame_that_is_gone_is_stale() {
    let transport = ScriptedTransport::new().expect(
        "Page.getFrameTree",
        json!({ "frameTree": { "frame": { "id": "OTHER", "loaderId": "L" } } }),
    );
    let mut connection = CdpConnection::new(transport.boxed());

    let error =
        call_on_element(&mut connection, &session(), &token(77), echo(), deadline()).unwrap_err();

    assert_eq!(error.code, ErrorCode::StaleRef);
}

#[test]
fn a_token_from_another_target_is_refused_without_a_request() {
    let mut connection = CdpConnection::new(ScriptedTransport::new().boxed());
    let mut foreign = token(77);
    foreign.target_id = "ELSEWHERE".into();

    let error =
        call_on_element(&mut connection, &session(), &foreign, echo(), deadline()).unwrap_err();

    assert_eq!(error.code, ErrorCode::InvalidArgs);
}

#[test]
fn a_transport_timeout_is_not_mistaken_for_staleness() {
    let transport = proven(ScriptedTransport::new()).expect_silence("DOM.resolveNode");
    let mut connection = CdpConnection::new(transport.boxed());

    let error = call_on_element(
        &mut connection,
        &session(),
        &token(77),
        echo(),
        Deadline::after(30).unwrap(),
    )
    .unwrap_err();

    assert_eq!(error.code, ErrorCode::Timeout);
}

#[test]
fn element_calls_bind_the_resolved_object_and_release_it() {
    let transport = resolved(ScriptedTransport::new())
        .expect(
            "Runtime.callFunctionOn",
            json!({ "result": { "type": "string", "value": "ok" } }),
        )
        .expect("Runtime.releaseObject", json!({}));
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());

    let value =
        call_on_element(&mut connection, &session(), &token(5), echo(), deadline()).unwrap();

    assert_eq!(value, json!("ok"));
    let sent = sent.lock().unwrap();
    assert_eq!(sent[1]["params"]["backendNodeId"], 5);
    assert_eq!(sent[2]["params"]["objectId"], "O");
    assert_eq!(sent[2]["params"]["arguments"], json!([{ "value": "ok" }]));
    assert_eq!(sent[2]["params"]["returnByValue"], true);
    assert_eq!(sent[3]["params"]["objectId"], "O");
    assert!(
        sent.iter()
            .all(|request| request["method"] != "Runtime.enable")
    );
}

#[test]
fn a_throwing_script_is_an_action_failure_and_still_releases() {
    let transport = resolved(ScriptedTransport::new())
        .expect(
            "Runtime.callFunctionOn",
            json!({ "result": {}, "exceptionDetails": { "text": "Uncaught", "exception": { "description": "TypeError: boom" } } }),
        )
        .expect("Runtime.releaseObject", json!({}));
    let mut connection = CdpConnection::new(transport.boxed());

    let error =
        call_on_element(&mut connection, &session(), &token(5), echo(), deadline()).unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.platform_detail.as_deref(), Some("TypeError: boom"));
}
