use agent_desktop_core::{ErrorCode, Point};
use serde_json::{Value, json};

use super::press_point;
use crate::CdpConnection;
use crate::act::act_fixture::{deadline, metrics, session, token};
use crate::transport::scripted_transport::ScriptedTransport;

fn quads(page_y: f64, quads: Value) -> ScriptedTransport {
    ScriptedTransport::new()
        .expect("Page.getLayoutMetrics", metrics(page_y))
        .expect("DOM.getContentQuads", json!({ "quads": quads }))
}

fn covered(transport: ScriptedTransport, hit: i64) -> ScriptedTransport {
    transport
        .expect("DOM.getNodeForLocation", json!({ "backendNodeId": hit }))
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
        .expect("Runtime.releaseObject", json!({}))
}

fn hit_tests(sent: &[Value]) -> Vec<(i64, i64)> {
    sent.iter()
        .filter(|request| request["method"] == "DOM.getNodeForLocation")
        .map(|request| {
            (
                request["params"]["x"].as_i64().unwrap(),
                request["params"]["y"].as_i64().unwrap(),
            )
        })
        .collect()
}

#[test]
fn a_scrolled_page_is_hit_tested_at_the_document_position_of_the_viewport_point() {
    let transport = quads(1_800.0, json!([[20, 570, 70, 570, 70, 596, 20, 596]]))
        .expect("DOM.getNodeForLocation", json!({ "backendNodeId": 7 }));
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());

    let point = press_point(&mut connection, &session(), &token(7), deadline()).unwrap();

    assert_eq!(point, Point { x: 45.0, y: 583.0 });
    assert_eq!(hit_tests(&sent.lock().unwrap()), vec![(45, 2_383)]);
}

#[test]
fn a_wrapped_link_is_pressed_on_the_first_line_that_hits_it() {
    let lines = json!([
        [150, 10, 190, 10, 190, 26, 150, 26],
        [0, 58, 40, 58, 40, 74, 0, 74]
    ]);
    let transport = covered(quads(0.0, lines), 9)
        .expect("DOM.getNodeForLocation", json!({ "backendNodeId": 7 }));
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());

    let point = press_point(&mut connection, &session(), &token(7), deadline()).unwrap();

    assert_eq!(point, Point { x: 20.0, y: 66.0 });
    assert_eq!(hit_tests(&sent.lock().unwrap()), vec![(170, 18), (20, 66)]);
}

#[test]
fn a_node_covered_on_every_line_is_refused_with_the_first_refusal() {
    let lines = json!([
        [150, 10, 190, 10, 190, 26, 150, 26],
        [0, 58, 40, 58, 40, 74, 0, 74]
    ]);
    let transport = covered(covered(quads(0.0, lines), 9), 12);
    let mut connection = CdpConnection::new(transport.boxed());

    let error = press_point(&mut connection, &session(), &token(7), deadline()).unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionFailed);
    let details = error.details.unwrap();
    assert_eq!(details["kind"], "cdp_click_intercepted");
    assert_eq!(details["hit_backend_node_id"], 9);
    assert_eq!(details["point"], json!({ "x": 170.0, "y": 18.0 }));
}

#[test]
fn only_the_visible_part_of_a_tall_node_is_pressed() {
    let transport = quads(0.0, json!([[0, 400, 100, 400, 100, 1_000, 0, 1_000]]))
        .expect("DOM.getNodeForLocation", json!({ "backendNodeId": 7 }));
    let mut connection = CdpConnection::new(transport.boxed());

    let point = press_point(&mut connection, &session(), &token(7), deadline()).unwrap();

    assert_eq!(point, Point { x: 50.0, y: 500.0 });
}

#[test]
fn a_node_with_no_boxes_has_nothing_to_press() {
    let mut connection = CdpConnection::new(quads(0.0, json!([])).boxed());

    let error = press_point(&mut connection, &session(), &token(7), deadline()).unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.details.unwrap()["kind"], "cdp_node_not_rendered");
}
