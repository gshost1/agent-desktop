use agent_desktop_core::Deadline;
use serde_json::json;

use super::scroll_by;
use crate::transport::scripted_transport::ScriptedTransport;
use crate::{CdpConnection, PageSession};

#[test]
fn the_scroll_reports_whether_the_position_moved() {
    let transport = ScriptedTransport::new()
        .expect("DOM.resolveNode", json!({ "object": { "objectId": "o" } }))
        .expect(
            "Runtime.callFunctionOn",
            json!({ "result": { "type": "boolean", "value": true } }),
        )
        .expect("Runtime.releaseObject", json!({}));
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());
    let session = PageSession {
        target_id: "T".into(),
        session_id: "S".into(),
    };
    let moved = scroll_by(
        &mut connection,
        &session,
        4,
        (0.0, 300.0),
        Deadline::after(500).unwrap(),
    )
    .unwrap();
    assert!(moved);
    let sent = sent.lock().unwrap();
    assert_eq!(sent[1]["params"]["arguments"][1]["value"], 300.0);
}
