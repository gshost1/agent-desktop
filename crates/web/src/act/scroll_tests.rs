use agent_desktop_core::{Deadline, ErrorCode, Point};
use serde_json::json;

use super::quad_center;
use crate::transport::scripted_transport::ScriptedTransport;
use crate::{CdpConnection, PageSession, click};

fn session() -> PageSession {
    PageSession {
        target_id: "T".into(),
        session_id: "S".into(),
    }
}

#[test]
fn the_center_of_a_quad_is_the_mean_of_its_corners() {
    let center = quad_center(Some(&json!([10, 20, 110, 20, 110, 60, 10, 60]))).unwrap();
    assert_eq!(center, Point { x: 60.0, y: 40.0 });
    assert!(quad_center(Some(&json!([5, 5, 5, 5, 5, 5, 5, 5]))).is_none());
    assert!(quad_center(Some(&json!([1, 2, 3]))).is_none());
    assert!(quad_center(None).is_none());
}

#[test]
fn click_presses_and_releases_at_the_box_center_without_raising_the_tab() {
    let transport = ScriptedTransport::new()
        .expect("DOM.resolveNode", json!({ "object": { "objectId": "O" } }))
        .expect("Runtime.releaseObject", json!({}))
        .expect("DOM.scrollIntoViewIfNeeded", json!({}))
        .expect(
            "DOM.getBoxModel",
            json!({ "model": { "border": [0, 0, 100, 0, 100, 40, 0, 40] } }),
        )
        .expect("Input.dispatchMouseEvent", json!({}))
        .expect("Input.dispatchMouseEvent", json!({}));
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());

    let point = click(
        &mut connection,
        &session(),
        9,
        Deadline::after(1_000).unwrap(),
    )
    .unwrap();

    assert_eq!(point, Point { x: 50.0, y: 20.0 });
    let sent = sent.lock().unwrap();
    assert_eq!(sent[4]["params"]["type"], "mousePressed");
    assert_eq!(sent[4]["params"]["x"], 50.0);
    assert_eq!(sent[5]["params"]["type"], "mouseReleased");
    assert_eq!(sent[5]["params"]["button"], "left");
}

#[test]
fn a_hidden_element_has_no_box_to_click() {
    let transport = ScriptedTransport::new()
        .expect("DOM.resolveNode", json!({ "object": { "objectId": "O" } }))
        .expect("Runtime.releaseObject", json!({}))
        .expect("DOM.scrollIntoViewIfNeeded", json!({}))
        .expect_error("DOM.getBoxModel", -32000, "Could not compute box model.");
    let mut connection = CdpConnection::new(transport.boxed());

    let error = click(
        &mut connection,
        &session(),
        9,
        Deadline::after(1_000).unwrap(),
    )
    .unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.details.as_ref().unwrap()["kind"],
        "cdp_node_not_rendered"
    );
}
