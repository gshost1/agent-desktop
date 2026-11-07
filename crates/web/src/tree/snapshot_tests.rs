use agent_desktop_core::{Deadline, ErrorCode};
use serde_json::{Value, json};

use super::snapshot_page;
use crate::transport::scripted_transport::ScriptedTransport;
use crate::{CdpConnection, PageSession};

fn session() -> PageSession {
    PageSession {
        target_id: "T".into(),
        session_id: "S".into(),
    }
}

fn role(name: &str) -> Value {
    json!({ "type": "role", "value": name })
}

fn text(value: &str) -> Value {
    json!({ "type": "computedString", "value": value })
}

fn frame_tree() -> Value {
    json!({ "frameTree": {
        "frame": { "id": "MAIN" },
        "childFrames": [
            { "frame": { "id": "CHILD", "parentId": "MAIN" } },
            { "frame": { "id": "REMOTE", "parentId": "MAIN" } },
        ]
    } })
}

fn main_tree() -> Value {
    json!({ "nodes": [
        { "nodeId": "1", "role": role("RootWebArea"), "name": text("Top"), "childIds": ["2", "3"], "backendDOMNodeId": 1 },
        { "nodeId": "2", "role": role("button"), "name": text("Go"), "backendDOMNodeId": 20 },
        { "nodeId": "3", "role": role("Iframe"), "backendDOMNodeId": 30 },
    ] })
}

fn child_tree() -> Value {
    json!({ "nodes": [
        { "nodeId": "1", "role": role("RootWebArea"), "name": text("Inner"), "childIds": ["2"], "backendDOMNodeId": 100 },
        { "nodeId": "2", "role": role("link"), "name": text("More"), "backendDOMNodeId": 101 },
    ] })
}

fn document() -> Value {
    json!({ "root": { "backendNodeId": 1, "children": [
        { "backendNodeId": 20, "attributes": ["id", "go", "class", "primary"] },
        { "backendNodeId": 30, "attributes": ["src", "x.html"], "contentDocument": { "backendNodeId": 99,
            "children": [{ "backendNodeId": 101, "attributes": ["id", "more"] }] } },
    ] } })
}

#[test]
fn frames_are_grafted_under_their_owner_and_origins_follow_preorder() {
    let transport = ScriptedTransport::new()
        .expect("Page.getFrameTree", frame_tree())
        .expect("DOM.getDocument", document())
        .expect("Accessibility.getFullAXTree", main_tree())
        .expect("Accessibility.getFullAXTree", child_tree())
        .expect("DOM.getFrameOwner", json!({ "backendNodeId": 30 }))
        .expect_error(
            "Accessibility.getFullAXTree",
            -32000,
            "Frame with the given id was not found.",
        );
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());

    let snapshot =
        snapshot_page(&mut connection, &session(), Deadline::after(1_000).unwrap()).unwrap();

    assert_eq!(snapshot.unreadable_frames, ["REMOTE"]);
    let preorder = snapshot.preorder();
    let summary: Vec<(&str, Option<&str>, Option<i64>)> = preorder
        .iter()
        .map(|(node, origin)| {
            (
                node.role.as_str(),
                node.identity.name.as_deref(),
                origin.map(|origin| origin.backend_node_id),
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            ("webarea", Some("Top"), Some(1)),
            ("button", Some("Go"), Some(20)),
            ("group", None, Some(30)),
            ("webarea", Some("Inner"), Some(100)),
            ("link", Some("More"), Some(101)),
        ]
    );
    assert_eq!(preorder[4].1.unwrap().frame_id, "CHILD");
    assert_eq!(
        preorder[1].0.identity.native_id.as_ref().unwrap().value,
        "go"
    );
    assert_eq!(
        preorder[4].0.identity.native_id.as_ref().unwrap().value,
        "more"
    );
    assert_eq!(
        preorder[4].0.identity.retained_object.as_deref(),
        Some("cdp:T:101")
    );
    let sent = sent.lock().unwrap();
    assert_eq!(sent[2]["params"]["frameId"], "MAIN");
    assert_eq!(sent[1]["params"], json!({ "depth": -1, "pierce": true }));
}

#[test]
fn an_unreadable_main_frame_fails_the_snapshot() {
    let transport = ScriptedTransport::new()
        .expect(
            "Page.getFrameTree",
            json!({ "frameTree": { "frame": { "id": "MAIN" } } }),
        )
        .expect("DOM.getDocument", json!({ "root": {} }))
        .expect_error(
            "Accessibility.getFullAXTree",
            -32000,
            "Accessibility is not enabled",
        );
    let mut connection = CdpConnection::new(transport.boxed());

    let error =
        snapshot_page(&mut connection, &session(), Deadline::after(1_000).unwrap()).unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionFailed);
}

#[test]
fn an_empty_main_frame_is_a_structured_error() {
    let transport = ScriptedTransport::new()
        .expect(
            "Page.getFrameTree",
            json!({ "frameTree": { "frame": { "id": "MAIN" } } }),
        )
        .expect("DOM.getDocument", json!({ "root": {} }))
        .expect("Accessibility.getFullAXTree", json!({ "nodes": [] }));
    let mut connection = CdpConnection::new(transport.boxed());

    let error =
        snapshot_page(&mut connection, &session(), Deadline::after(1_000).unwrap()).unwrap_err();

    assert_eq!(error.details.as_ref().unwrap()["kind"], "cdp_tree_empty");
}
