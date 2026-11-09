use serde_json::json;

use super::scroll_by;
use crate::CdpConnection;
use crate::act::act_fixture::{deadline, resolved, session, token};
use crate::transport::scripted_transport::ScriptedTransport;

#[test]
fn the_scroll_reports_whether_the_position_moved() {
    let transport = resolved(ScriptedTransport::new())
        .expect(
            "Runtime.callFunctionOn",
            json!({ "result": { "type": "boolean", "value": true } }),
        )
        .expect("Runtime.releaseObject", json!({}));
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());
    let moved = scroll_by(
        &mut connection,
        &session(),
        &token(4),
        (0.0, 300.0),
        deadline(),
    )
    .unwrap();
    assert!(moved);
    let sent = sent.lock().unwrap();
    assert_eq!(sent[2]["params"]["arguments"][1]["value"], 300.0);
}
