use agent_desktop_core::Deadline;
use serde_json::json;

use super::page_visibility;
use crate::transport::scripted_transport::ScriptedTransport;
use crate::{CdpConnection, PageSession};

fn session() -> PageSession {
    PageSession {
        target_id: "T".into(),
        session_id: "S".into(),
    }
}

#[test]
fn visibility_is_read_in_an_isolated_world_without_enabling_the_runtime() {
    let transport = ScriptedTransport::new()
        .expect(
            "Page.getFrameTree",
            json!({ "frameTree": { "frame": { "id": "F1" } } }),
        )
        .expect(
            "Page.createIsolatedWorld",
            json!({ "executionContextId": 9 }),
        )
        .expect(
            "Runtime.evaluate",
            json!({ "result": { "type": "string", "value": "hidden" } }),
        );
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());
    let state =
        page_visibility(&mut connection, &session(), Deadline::after(500).unwrap()).unwrap();
    assert_eq!(state, "hidden");
    let sent = sent.lock().unwrap();
    assert_eq!(sent[1]["params"]["frameId"], "F1");
    assert_eq!(sent[2]["params"]["contextId"], 9);
    assert!(
        sent.iter()
            .all(|request| request["method"] != "Runtime.enable")
    );
}

#[test]
fn a_reply_without_a_main_frame_is_an_error() {
    let transport = ScriptedTransport::new().expect("Page.getFrameTree", json!({}));
    let mut connection = CdpConnection::new(transport.boxed());
    assert!(page_visibility(&mut connection, &session(), Deadline::after(500).unwrap()).is_err());
}
