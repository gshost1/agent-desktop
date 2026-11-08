use agent_desktop_core::ErrorCode;
use serde_json::{Value, json};

use super::set_checked;
use crate::CdpConnection;
use crate::act::act_fixture::{deadline, laid_out, resolved, session, token};
use crate::transport::scripted_transport::ScriptedTransport;

fn read(transport: ScriptedTransport, checked: Value) -> ScriptedTransport {
    transport.expect(
        "Runtime.callFunctionOn",
        json!({ "result": { "value": { "value": "on", "text": null, "checked": checked } } }),
    )
}

fn click(transport: ScriptedTransport) -> ScriptedTransport {
    laid_out(
        transport.expect("DOM.scrollIntoViewIfNeeded", json!({})),
        json!([[0, 0, 10, 0, 10, 10, 0, 10]]),
    )
    .expect("DOM.getNodeForLocation", json!({ "backendNodeId": 3 }))
    .expect("Input.dispatchMouseEvent", json!({}))
    .expect("Input.dispatchMouseEvent", json!({}))
}

fn released(transport: ScriptedTransport) -> ScriptedTransport {
    transport.expect("Runtime.releaseObject", json!({}))
}

#[test]
fn an_already_checked_box_is_not_clicked() {
    let transport = released(read(resolved(ScriptedTransport::new()), json!(true)));
    let mut connection = CdpConnection::new(transport.boxed());
    assert!(set_checked(&mut connection, &session(), &token(3), true, deadline()).unwrap());
}

#[test]
fn a_differing_box_is_clicked_and_verified_with_one_resolve() {
    let transport = released(read(
        click(read(resolved(ScriptedTransport::new()), json!(false))),
        json!(true),
    ));
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());

    assert!(set_checked(&mut connection, &session(), &token(3), true, deadline()).unwrap());

    let sent = sent.lock().unwrap();
    assert_eq!(sent.len(), 11);
    let resolves = sent
        .iter()
        .filter(|request| request["method"] == "DOM.resolveNode")
        .count();
    assert_eq!(resolves, 1);
}

#[test]
fn a_click_that_does_not_change_state_fails() {
    let transport = released(read(
        click(read(resolved(ScriptedTransport::new()), json!(true))),
        json!(true),
    ));
    let mut connection = CdpConnection::new(transport.boxed());
    let error = set_checked(&mut connection, &session(), &token(3), false, deadline()).unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionFailed);
}

#[test]
fn a_node_without_checked_state_is_not_supported() {
    let transport = released(read(resolved(ScriptedTransport::new()), Value::Null));
    let mut connection = CdpConnection::new(transport.boxed());
    let error = set_checked(&mut connection, &session(), &token(3), true, deadline()).unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionNotSupported);
}
