use agent_desktop_core::Deadline;
use serde_json::json;

use super::append_text;
use crate::transport::scripted_transport::ScriptedTransport;
use crate::{CdpConnection, PageSession};

fn session() -> PageSession {
    PageSession {
        target_id: "T".into(),
        session_id: "S".into(),
    }
}

fn caret_moved() -> ScriptedTransport {
    ScriptedTransport::new()
        .expect("DOM.resolveNode", json!({ "object": { "objectId": "o1" } }))
        .expect("Runtime.releaseObject", json!({}))
        .expect("DOM.focus", json!({}))
        .expect("DOM.resolveNode", json!({ "object": { "objectId": "o2" } }))
        .expect(
            "Runtime.callFunctionOn",
            json!({ "result": { "type": "string", "value": "control" } }),
        )
        .expect("Runtime.releaseObject", json!({}))
}

#[test]
fn text_is_inserted_after_the_caret_moves_to_the_end() {
    let transport = caret_moved().expect("Input.insertText", json!({}));
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());
    append_text(
        &mut connection,
        &session(),
        5,
        " world",
        Deadline::after(500).unwrap(),
    )
    .unwrap();
    let sent = sent.lock().unwrap();
    let insert = sent.last().unwrap();
    assert_eq!(insert["params"]["text"], " world");
    assert!(
        sent[4]["params"]["functionDeclaration"]
            .as_str()
            .unwrap()
            .contains("setSelectionRange(end, end)")
    );
}

#[test]
fn empty_text_only_places_the_caret() {
    let mut connection = CdpConnection::new(caret_moved().boxed());
    append_text(
        &mut connection,
        &session(),
        5,
        "",
        Deadline::after(500).unwrap(),
    )
    .unwrap();
}
