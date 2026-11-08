use agent_desktop_core::ErrorCode;
use serde_json::json;

use super::read_node;
use crate::CdpConnection;
use crate::act::act_fixture::{deadline, proven, session, token};
use crate::transport::scripted_transport::ScriptedTransport;

fn live() -> ScriptedTransport {
    proven(ScriptedTransport::new())
}

#[test]
fn a_live_node_reads_back_in_snapshot_form_with_its_token() {
    let transport = live().expect(
        "Accessibility.getPartialAXTree",
        json!({ "nodes": [{
            "nodeId": "1", "backendDOMNodeId": 12, "ignored": false,
            "role": { "type": "role", "value": "checkbox" },
            "name": { "type": "computedString", "value": "Agree" },
            "properties": [
                { "name": "checked", "value": { "type": "tristate", "value": "true" } },
                { "name": "focusable", "value": { "type": "booleanOrUndefined", "value": true } }
            ]
        }] }),
    );
    let mut connection = CdpConnection::new(transport.boxed());
    let node = read_node(&mut connection, &session(), &token(12), deadline()).unwrap();
    assert_eq!(node.role, "checkbox");
    assert_eq!(node.identity.name.as_deref(), Some("Agree"));
    assert_eq!(
        node.identity.retained_object.as_deref(),
        Some("cdp:T:F:L:12")
    );
    assert!(
        node.presentation
            .states
            .iter()
            .any(|state| state == "checked")
    );
    assert!(
        node.presentation
            .available_actions
            .iter()
            .any(|action| action == "Toggle")
    );
}

#[test]
fn an_ignored_node_reads_as_hidden() {
    let transport = live().expect(
        "Accessibility.getPartialAXTree",
        json!({ "nodes": [{
            "nodeId": "1", "backendDOMNodeId": 12, "ignored": true,
            "role": { "type": "role", "value": "button" },
            "name": { "type": "computedString", "value": "Go" }
        }] }),
    );
    let mut connection = CdpConnection::new(transport.boxed());
    let node = read_node(&mut connection, &session(), &token(12), deadline()).unwrap();
    assert!(
        node.presentation
            .states
            .iter()
            .any(|state| state == "hidden")
    );
}

#[test]
fn a_node_the_page_dropped_is_stale() {
    let transport = live().expect_error(
        "Accessibility.getPartialAXTree",
        -32000,
        "No node with given id found",
    );
    let mut connection = CdpConnection::new(transport.boxed());
    let error = read_node(&mut connection, &session(), &token(12), deadline()).unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleRef);
}

#[test]
fn a_node_of_a_replaced_document_is_stale_before_it_is_read() {
    let transport = ScriptedTransport::new().expect(
        "Page.getFrameTree",
        json!({ "frameTree": { "frame": { "id": "F", "loaderId": "L2" } } }),
    );
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());
    let error = read_node(&mut connection, &session(), &token(12), deadline()).unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(
        error.details.as_ref().unwrap()["kind"],
        "cdp_document_replaced"
    );
    assert_eq!(sent.lock().unwrap().len(), 1);
}

fn generic_row() -> serde_json::Value {
    json!({ "nodes": [{
        "nodeId": "1", "backendDOMNodeId": 12, "ignored": false,
        "role": { "type": "role", "value": "generic" }
    }] })
}

#[test]
fn a_plain_node_whose_dom_node_listens_for_clicks_reads_back_clickable() {
    let transport = live()
        .expect("Accessibility.getPartialAXTree", generic_row())
        .expect("DOM.resolveNode", json!({ "object": { "objectId": "O" } }))
        .expect(
            "DOMDebugger.getEventListeners",
            json!({ "listeners": [{ "type": "click" }] }),
        )
        .expect("Runtime.releaseObjectGroup", json!({}));
    let mut connection = CdpConnection::new(transport.boxed());
    let node = read_node(&mut connection, &session(), &token(12), deadline()).unwrap();
    assert_eq!(node.role, "group");
    assert_eq!(node.presentation.available_actions, ["Click"]);
    assert_eq!(
        node.identity.retained_object.as_deref(),
        Some("cdp:T:F:L:12")
    );
}

#[test]
fn a_plain_node_without_a_click_listener_reads_back_inert() {
    let transport = live()
        .expect("Accessibility.getPartialAXTree", generic_row())
        .expect("DOM.resolveNode", json!({ "object": { "objectId": "O" } }))
        .expect("DOMDebugger.getEventListeners", json!({ "listeners": [] }))
        .expect("Runtime.releaseObjectGroup", json!({}));
    let mut connection = CdpConnection::new(transport.boxed());
    let node = read_node(&mut connection, &session(), &token(12), deadline()).unwrap();
    assert!(node.presentation.available_actions.is_empty());
    assert!(node.identity.retained_object.is_none());
}
