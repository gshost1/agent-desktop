use agent_desktop_core::Deadline;
use serde_json::json;

use super::listens_for_clicks;
use crate::transport::scripted_transport::ScriptedTransport;
use crate::{CdpConnection, PageSession};

fn session() -> PageSession {
    PageSession {
        target_id: "T".into(),
        session_id: "S".into(),
    }
}

fn listens(transport: ScriptedTransport) -> (bool, Vec<String>) {
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());
    let listens = listens_for_clicks(
        &mut connection,
        &session(),
        7,
        Deadline::after(1_000).unwrap(),
    )
    .unwrap();
    let methods = sent
        .lock()
        .unwrap()
        .iter()
        .filter_map(|frame| frame["method"].as_str().map(str::to_owned))
        .collect();
    (listens, methods)
}

fn with_listeners(listeners: serde_json::Value) -> ScriptedTransport {
    ScriptedTransport::new()
        .expect("DOM.resolveNode", json!({ "object": { "objectId": "O" } }))
        .expect(
            "DOMDebugger.getEventListeners",
            json!({ "listeners": listeners }),
        )
        .expect("Runtime.releaseObjectGroup", json!({}))
}

#[test]
fn a_click_mousedown_or_mouseup_listener_counts_and_the_object_is_released() {
    for kind in ["click", "mousedown", "mouseup"] {
        let (listens, methods) = listens(with_listeners(json!([{ "type": kind }])));
        assert!(listens, "{kind}");
        assert_eq!(
            methods,
            [
                "DOM.resolveNode",
                "DOMDebugger.getEventListeners",
                "Runtime.releaseObjectGroup"
            ]
        );
    }
}

#[test]
fn other_listeners_and_no_listeners_do_not_count() {
    let (listens_other, _) = listens(with_listeners(
        json!([{ "type": "pointerdown" }, { "type": "keydown" }]),
    ));
    assert!(!listens_other);
    let (listens_none, _) = listens(with_listeners(json!([])));
    assert!(!listens_none);
}

#[test]
fn a_node_the_page_cannot_resolve_counts_as_no_listener() {
    let transport =
        ScriptedTransport::new().expect_error("DOM.resolveNode", -32000, "No node with given id");
    let (listens, methods) = listens(transport);
    assert!(!listens);
    assert_eq!(methods, ["DOM.resolveNode"]);
}
