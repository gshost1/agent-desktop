use agent_desktop_core::Deadline;
use serde_json::json;

use crate::transport::scripted_transport::ScriptedTransport;
use crate::{ElementToken, PageSession};

pub(crate) fn session() -> PageSession {
    PageSession {
        target_id: "T".into(),
        session_id: "S".into(),
    }
}

pub(crate) fn token(backend_node_id: i64) -> ElementToken {
    ElementToken {
        target_id: "T".into(),
        frame_id: "F".into(),
        loader_id: "L".into(),
        backend_node_id,
    }
}

pub(crate) fn deadline() -> Deadline {
    Deadline::after(1_000).unwrap()
}

pub(crate) fn proven(transport: ScriptedTransport) -> ScriptedTransport {
    transport.expect(
        "Page.getFrameTree",
        json!({ "frameTree": { "frame": { "id": "F", "loaderId": "L" } } }),
    )
}

pub(crate) fn resolved(transport: ScriptedTransport) -> ScriptedTransport {
    proven(transport).expect("DOM.resolveNode", json!({ "object": { "objectId": "O" } }))
}
