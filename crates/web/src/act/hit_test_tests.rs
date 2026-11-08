use agent_desktop_core::{DeliverySemantics, ErrorCode, Point};
use serde_json::json;

use super::confirm_hit;
use crate::CdpConnection;
use crate::act::act_fixture::{deadline, session, token};
use crate::act::hit_point::HitPoint;
use crate::transport::scripted_transport::ScriptedTransport;

fn point() -> HitPoint {
    HitPoint {
        viewport: Point { x: 40.4, y: 19.6 },
        document: Point {
            x: 40.4,
            y: 1_219.6,
        },
    }
}

fn located(hit: serde_json::Value) -> ScriptedTransport {
    ScriptedTransport::new().expect("DOM.getNodeForLocation", hit)
}

fn compared(transport: ScriptedTransport, inside: bool) -> ScriptedTransport {
    transport
        .expect(
            "DOM.resolveNode",
            json!({ "object": { "objectId": "target" } }),
        )
        .expect(
            "DOM.resolveNode",
            json!({ "object": { "objectId": "hit" } }),
        )
        .expect(
            "Runtime.callFunctionOn",
            json!({ "result": { "type": "boolean", "value": inside } }),
        )
        .expect("Runtime.releaseObject", json!({}))
        .expect("Runtime.releaseObject", json!({}))
}

#[test]
fn a_hit_on_the_node_itself_passes_in_one_round_trip() {
    let transport = located(json!({ "backendNodeId": 7, "frameId": "F" }));
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());

    confirm_hit(&mut connection, &session(), &token(7), &point(), deadline()).unwrap();

    let sent = sent.lock().unwrap();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0]["params"]["x"], 40);
    assert_eq!(sent[0]["params"]["y"], 1_220);
    assert_eq!(sent[0]["sessionId"], "S");
}

#[test]
fn a_hit_on_a_descendant_passes() {
    let transport = compared(located(json!({ "backendNodeId": 9 })), true);
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());

    confirm_hit(&mut connection, &session(), &token(7), &point(), deadline()).unwrap();

    let sent = sent.lock().unwrap();
    assert_eq!(sent[1]["params"]["backendNodeId"], 7);
    assert_eq!(sent[2]["params"]["backendNodeId"], 9);
    assert_eq!(sent[3]["params"]["objectId"], "target");
    assert_eq!(sent[3]["params"]["arguments"][0]["objectId"], "hit");
}

#[test]
fn a_covering_element_is_refused_as_not_delivered() {
    let mut connection =
        CdpConnection::new(compared(located(json!({ "backendNodeId": 9 })), false).boxed());

    let error =
        confirm_hit(&mut connection, &session(), &token(7), &point(), deadline()).unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionFailed);
    let details = error.details.as_ref().unwrap();
    assert_eq!(details["kind"], "cdp_click_intercepted");
    assert_eq!(details["backend_node_id"], 7);
    assert_eq!(details["hit_backend_node_id"], 9);
    assert_eq!(details["point"], json!({ "x": 40.4, "y": 19.6 }));
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn a_point_with_no_node_is_refused() {
    let transport = ScriptedTransport::new().expect_error(
        "DOM.getNodeForLocation",
        -32000,
        "No node found at given location",
    );
    let mut connection = CdpConnection::new(transport.boxed());

    let error =
        confirm_hit(&mut connection, &session(), &token(7), &point(), deadline()).unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.details.as_ref().unwrap()["hit_backend_node_id"],
        json!(null)
    );
}

#[test]
fn a_hit_in_another_script_world_counts_as_covered() {
    let transport = located(json!({ "backendNodeId": 9 }))
        .expect(
            "DOM.resolveNode",
            json!({ "object": { "objectId": "target" } }),
        )
        .expect(
            "DOM.resolveNode",
            json!({ "object": { "objectId": "hit" } }),
        )
        .expect_error(
            "Runtime.callFunctionOn",
            -32000,
            "Argument should belong to the same JavaScript world as target object",
        )
        .expect("Runtime.releaseObject", json!({}))
        .expect("Runtime.releaseObject", json!({}));
    let mut connection = CdpConnection::new(transport.boxed());

    let error =
        confirm_hit(&mut connection, &session(), &token(7), &point(), deadline()).unwrap_err();

    assert_eq!(
        error.details.as_ref().unwrap()["kind"],
        "cdp_click_intercepted"
    );
}
