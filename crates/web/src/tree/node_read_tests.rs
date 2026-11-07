use agent_desktop_core::{Deadline, ErrorCode};
use serde_json::json;

use super::read_node;
use crate::transport::scripted_transport::ScriptedTransport;
use crate::{CdpConnection, PageSession};

fn session() -> PageSession {
    PageSession {
        target_id: "T".into(),
        session_id: "S".into(),
    }
}

fn live() -> ScriptedTransport {
    ScriptedTransport::new()
        .expect("DOM.resolveNode", json!({ "object": { "objectId": "o" } }))
        .expect("Runtime.releaseObject", json!({}))
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
    let node = read_node(
        &mut connection,
        &session(),
        12,
        Deadline::after(500).unwrap(),
    )
    .unwrap();
    assert_eq!(node.role, "checkbox");
    assert_eq!(node.identity.name.as_deref(), Some("Agree"));
    assert_eq!(node.identity.retained_object.as_deref(), Some("cdp:T:12"));
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
    let node = read_node(
        &mut connection,
        &session(),
        12,
        Deadline::after(500).unwrap(),
    )
    .unwrap();
    assert!(
        node.presentation
            .states
            .iter()
            .any(|state| state == "hidden")
    );
}

#[test]
fn a_node_the_page_dropped_is_stale() {
    let transport = ScriptedTransport::new().expect_error(
        "DOM.resolveNode",
        -32000,
        "No node with given id found",
    );
    let mut connection = CdpConnection::new(transport.boxed());
    let error = read_node(
        &mut connection,
        &session(),
        12,
        Deadline::after(500).unwrap(),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleRef);
}
