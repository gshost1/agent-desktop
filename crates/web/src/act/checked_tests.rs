use agent_desktop_core::{Deadline, ErrorCode};
use serde_json::{Value, json};

use super::set_checked;
use crate::transport::scripted_transport::ScriptedTransport;
use crate::{CdpConnection, PageSession};

fn session() -> PageSession {
    PageSession {
        target_id: "T".into(),
        session_id: "S".into(),
    }
}

fn read(transport: ScriptedTransport, checked: Value) -> ScriptedTransport {
    transport
        .expect("DOM.resolveNode", json!({ "object": { "objectId": "O" } }))
        .expect(
            "Runtime.callFunctionOn",
            json!({ "result": { "value": { "value": "on", "text": null, "checked": checked } } }),
        )
        .expect("Runtime.releaseObject", json!({}))
}

fn click(transport: ScriptedTransport) -> ScriptedTransport {
    transport
        .expect("DOM.resolveNode", json!({ "object": { "objectId": "O" } }))
        .expect("Runtime.releaseObject", json!({}))
        .expect("DOM.scrollIntoViewIfNeeded", json!({}))
        .expect(
            "DOM.getBoxModel",
            json!({ "model": { "border": [0, 0, 10, 0, 10, 10, 0, 10] } }),
        )
        .expect("Input.dispatchMouseEvent", json!({}))
        .expect("Input.dispatchMouseEvent", json!({}))
}

fn deadline() -> Deadline {
    Deadline::after(1_000).unwrap()
}

#[test]
fn an_already_checked_box_is_not_clicked() {
    let transport = read(ScriptedTransport::new(), json!(true));
    let mut connection = CdpConnection::new(transport.boxed());
    assert!(set_checked(&mut connection, &session(), 3, true, deadline()).unwrap());
}

#[test]
fn a_differing_box_is_clicked_and_verified() {
    let transport = read(
        click(read(ScriptedTransport::new(), json!(false))),
        json!(true),
    );
    let mut connection = CdpConnection::new(transport.boxed());
    assert!(set_checked(&mut connection, &session(), 3, true, deadline()).unwrap());
}

#[test]
fn a_click_that_does_not_change_state_fails() {
    let transport = read(
        click(read(ScriptedTransport::new(), json!(true))),
        json!(true),
    );
    let mut connection = CdpConnection::new(transport.boxed());
    let error = set_checked(&mut connection, &session(), 3, false, deadline()).unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionFailed);
}

#[test]
fn a_node_without_checked_state_is_not_supported() {
    let transport = read(ScriptedTransport::new(), Value::Null);
    let mut connection = CdpConnection::new(transport.boxed());
    let error = set_checked(&mut connection, &session(), 3, true, deadline()).unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionNotSupported);
}
