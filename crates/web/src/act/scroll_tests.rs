use agent_desktop_core::{ErrorCode, Point};
use serde_json::json;

use super::quad_center;
use crate::act::act_fixture::{deadline, proven, session, token};
use crate::transport::scripted_transport::ScriptedTransport;
use crate::{CdpConnection, click, scroll_into_view};

#[test]
fn the_center_of_a_quad_is_the_mean_of_its_corners() {
    let center = quad_center(Some(&json!([10, 20, 110, 20, 110, 60, 10, 60]))).unwrap();
    assert_eq!(center, Point { x: 60.0, y: 40.0 });
    assert!(quad_center(Some(&json!([5, 5, 5, 5, 5, 5, 5, 5]))).is_none());
    assert!(quad_center(Some(&json!([1, 2, 3]))).is_none());
    assert!(quad_center(None).is_none());
}

#[test]
fn click_presses_and_releases_at_the_box_center_once_the_point_hits_the_node() {
    let transport = boxed_node()
        .expect("DOM.getNodeForLocation", json!({ "backendNodeId": 9 }))
        .expect("Input.dispatchMouseEvent", json!({}))
        .expect("Input.dispatchMouseEvent", json!({}));
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());

    let point = click(&mut connection, &session(), &token(9), deadline()).unwrap();

    assert_eq!(point, Point { x: 50.0, y: 20.0 });
    let sent = sent.lock().unwrap();
    assert_eq!(sent.len(), 6);
    assert_eq!(sent[3]["method"], "DOM.getNodeForLocation");
    assert_eq!(sent[4]["params"]["type"], "mousePressed");
    assert_eq!(sent[4]["params"]["x"], 50.0);
    assert_eq!(sent[5]["params"]["type"], "mouseReleased");
    assert_eq!(sent[5]["params"]["button"], "left");
}

#[test]
fn a_click_whose_point_another_element_covers_sends_no_input() {
    let transport = boxed_node()
        .expect("DOM.getNodeForLocation", json!({ "backendNodeId": 31 }))
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
            json!({ "result": { "type": "boolean", "value": false } }),
        )
        .expect("Runtime.releaseObject", json!({}))
        .expect("Runtime.releaseObject", json!({}));
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());

    let error = click(&mut connection, &session(), &token(9), deadline()).unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.details.as_ref().unwrap()["kind"],
        "cdp_click_intercepted"
    );
    assert!(
        sent.lock()
            .unwrap()
            .iter()
            .all(|request| request["method"] != "Input.dispatchMouseEvent")
    );
}

fn boxed_node() -> ScriptedTransport {
    proven(ScriptedTransport::new())
        .expect("DOM.scrollIntoViewIfNeeded", json!({}))
        .expect(
            "DOM.getBoxModel",
            json!({ "model": { "border": [0, 0, 100, 0, 100, 40, 0, 40] } }),
        )
}

#[test]
fn a_node_the_page_dropped_is_stale_at_the_scroll() {
    let transport = proven(ScriptedTransport::new()).expect_error(
        "DOM.scrollIntoViewIfNeeded",
        -32000,
        "No node with given id found",
    );
    let mut connection = CdpConnection::new(transport.boxed());

    let error = click(&mut connection, &session(), &token(9), deadline()).unwrap_err();

    assert_eq!(error.code, ErrorCode::StaleRef);
}

#[test]
fn a_lost_session_during_scroll_is_target_gone_not_stale() {
    let transport = proven(ScriptedTransport::new()).expect_error(
        "DOM.scrollIntoViewIfNeeded",
        -32001,
        "Session with given id not found.",
    );
    let mut connection = CdpConnection::new(transport.boxed());

    let error = scroll_into_view(&mut connection, &session(), &token(9), deadline()).unwrap_err();

    assert_eq!(error.code, ErrorCode::WindowNotFound);
    assert_eq!(error.details.as_ref().unwrap()["kind"], "cdp_target_gone");
}

#[test]
fn a_hidden_element_has_no_box_to_click() {
    let transport = proven(ScriptedTransport::new())
        .expect("DOM.scrollIntoViewIfNeeded", json!({}))
        .expect_error("DOM.getBoxModel", -32000, "Could not compute box model.");
    let mut connection = CdpConnection::new(transport.boxed());

    let error = click(&mut connection, &session(), &token(9), deadline()).unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.details.as_ref().unwrap()["kind"],
        "cdp_node_not_rendered"
    );
}
