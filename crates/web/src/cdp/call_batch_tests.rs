use agent_desktop_core::{Deadline, ErrorCode};
use serde_json::json;

use super::MAX_IN_FLIGHT;
use crate::CdpConnection;
use crate::cdp::cdp_request::CdpRequest;
use crate::cdp::protocol_error::is_protocol_error;
use crate::transport::scripted_transport::ScriptedTransport;

fn request(method: &'static str, session: &str) -> CdpRequest {
    CdpRequest {
        method,
        params: json!({}),
        session_id: Some(session.to_owned()),
        deadline: Deadline::after(1_000).unwrap(),
    }
}

#[test]
fn replies_arriving_out_of_order_come_back_in_request_order() {
    let transport = ScriptedTransport::new()
        .expect_silence("Page.getFrameTree")
        .expect_silence("DOMSnapshot.captureSnapshot")
        .expect_silence("Accessibility.getFullAXTree")
        .push_inbound(json!({ "id": 3, "result": { "n": 3 } }))
        .push_inbound(json!({ "id": 1, "result": { "n": 1 } }))
        .push_inbound(json!({ "id": 2, "result": { "n": 2 } }));
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());

    let replies = connection
        .call_batch(&[
            request("Page.getFrameTree", "S1"),
            request("DOMSnapshot.captureSnapshot", "S2"),
            request("Accessibility.getFullAXTree", "S3"),
        ])
        .unwrap();

    let numbers: Vec<i64> = replies
        .iter()
        .map(|reply| reply.as_ref().unwrap()["n"].as_i64().unwrap())
        .collect();
    assert_eq!(numbers, [1, 2, 3]);
    let sent = sent.lock().unwrap();
    assert_eq!(sent[2]["sessionId"], "S3");
}

#[test]
fn no_more_than_the_in_flight_bound_is_sent_before_a_reply_is_read() {
    let count = MAX_IN_FLIGHT + 1;
    let mut transport =
        ScriptedTransport::new().push_inbound(json!({ "id": count, "result": { "early": true } }));
    for id in 1..=count {
        transport = transport.expect_silence("Accessibility.getFullAXTree");
        if id < count {
            transport = transport.push_inbound(json!({ "id": id, "result": {} }));
        }
    }
    let mut connection = CdpConnection::new(transport.boxed());
    let requests: Vec<CdpRequest> = (0..count)
        .map(|_| request("Accessibility.getFullAXTree", "S"))
        .collect();

    let replies = connection.call_batch(&requests).unwrap();

    assert!(replies[..MAX_IN_FLIGHT].iter().all(Result::is_ok));
    assert_eq!(
        replies[MAX_IN_FLIGHT].as_ref().unwrap_err().code,
        ErrorCode::Timeout
    );
}

#[test]
fn a_silent_or_refused_request_fails_alone() {
    let transport = ScriptedTransport::new()
        .expect_silence("Page.getFrameTree")
        .expect_error("DOM.getFrameOwner", -32000, "Frame not found")
        .expect("Accessibility.getFullAXTree", json!({ "nodes": [] }));
    let mut connection = CdpConnection::new(transport.boxed());

    let replies = connection
        .call_batch(&[
            request("Page.getFrameTree", "S1"),
            request("DOM.getFrameOwner", "S2"),
            request("Accessibility.getFullAXTree", "S3"),
        ])
        .unwrap();

    assert_eq!(replies[0].as_ref().unwrap_err().code, ErrorCode::Timeout);
    assert!(is_protocol_error(replies[1].as_ref().unwrap_err()));
    assert_eq!(replies[2].as_ref().unwrap(), &json!({ "nodes": [] }));
}

#[test]
fn a_detached_session_fails_only_its_own_pending_requests() {
    let transport = ScriptedTransport::new()
        .expect_silence("Page.getFrameTree")
        .expect_with_events(
            "Page.getFrameTree",
            vec![json!({ "method": "Target.detachedFromTarget",
                "params": { "sessionId": "S1", "targetId": "T1" } })],
            json!({ "result": { "frameTree": {} } }),
        );
    let mut connection = CdpConnection::new(transport.boxed());

    let replies = connection
        .call_batch(&[
            request("Page.getFrameTree", "S1"),
            request("Page.getFrameTree", "S2"),
        ])
        .unwrap();

    assert_eq!(
        replies[0].as_ref().unwrap_err().code,
        ErrorCode::WindowNotFound
    );
    assert_eq!(replies[1].as_ref().unwrap(), &json!({ "frameTree": {} }));
    assert_eq!(connection.take_events().len(), 1);
}
