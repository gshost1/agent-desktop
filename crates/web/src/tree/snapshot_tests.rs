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
        "frame": { "id": "MAIN", "loaderId": "LM" },
        "childFrames": [
            { "frame": { "id": "CHILD", "parentId": "MAIN", "loaderId": "LC" } },
            { "frame": { "id": "REMOTE", "parentId": "MAIN", "loaderId": "LR" } },
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

fn capture() -> Value {
    json!({
        "strings": ["id", "go", "class", "primary", "src", "x.html", "more"],
        "documents": [
            { "nodes": { "backendNodeId": [1, 20, 30], "attributes": [[], [0, 1, 2, 3], [4, 5]] } },
            { "nodes": { "backendNodeId": [99, 101], "attributes": [[], [0, 6]] } },
        ],
    })
}

fn auto_attach(transport: ScriptedTransport, events: Vec<Value>) -> ScriptedTransport {
    transport.expect_with_events("Target.setAutoAttach", events, json!({ "result": {} }))
}

#[test]
fn frames_are_grafted_under_their_owner_and_origins_follow_preorder() {
    let transport = ScriptedTransport::new()
        .expect("Page.getFrameTree", frame_tree())
        .expect("DOMSnapshot.captureSnapshot", capture())
        .expect("Accessibility.getFullAXTree", main_tree())
        .expect("Accessibility.getFullAXTree", child_tree())
        .expect("DOM.getFrameOwner", json!({ "backendNodeId": 30 }))
        .expect_error(
            "Accessibility.getFullAXTree",
            -32000,
            "Frame with the given id was not found.",
        );
    let transport = auto_attach(transport, Vec::new()).expect("Target.setAutoAttach", json!({}));
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
        Some("cdp:T:CHILD:LC:101")
    );
    let sent = sent.lock().unwrap();
    assert_eq!(sent[2]["params"]["frameId"], "MAIN");
    assert_eq!(sent[1]["params"], json!({ "computedStyles": ["display"] }));
    assert_eq!(sent[6]["params"]["autoAttach"], true);
    assert_eq!(sent[7]["params"]["autoAttach"], false);
}

#[test]
fn an_unreadable_main_frame_fails_the_snapshot() {
    let transport = ScriptedTransport::new()
        .expect(
            "Page.getFrameTree",
            json!({ "frameTree": { "frame": { "id": "MAIN" } } }),
        )
        .expect("DOMSnapshot.captureSnapshot", json!({}))
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
        .expect("DOMSnapshot.captureSnapshot", json!({}))
        .expect("Accessibility.getFullAXTree", json!({ "nodes": [] }))
        .expect("Target.setAutoAttach", json!({}))
        .expect("Target.setAutoAttach", json!({}));
    let mut connection = CdpConnection::new(transport.boxed());

    let error =
        snapshot_page(&mut connection, &session(), Deadline::after(1_000).unwrap()).unwrap_err();

    assert_eq!(error.details.as_ref().unwrap()["kind"], "cdp_tree_empty");
}

fn attached(session_id: &str, target_id: &str) -> Value {
    json!({
        "method": "Target.attachedToTarget",
        "sessionId": "S",
        "params": {
            "sessionId": session_id,
            "targetInfo": { "targetId": target_id, "type": "iframe", "parentFrameId": "MAIN" },
            "waitingForDebugger": false,
        },
    })
}

#[test]
fn an_out_of_process_iframe_is_read_through_its_own_target_and_grafted() {
    let main_frames = json!({ "frameTree": { "frame": { "id": "MAIN", "loaderId": "LM" } } });
    let oopif_frames = json!({ "frameTree": {
        "frame": { "id": "OOP", "parentId": "MAIN", "loaderId": "LO" },
        "childFrames": [{ "frame": { "id": "OOP-CHILD", "parentId": "OOP", "loaderId": "LOC" } }],
    } });
    let transport = ScriptedTransport::new()
        .expect("Page.getFrameTree", main_frames)
        .expect("DOMSnapshot.captureSnapshot", json!({}))
        .expect("Accessibility.getFullAXTree", main_tree());
    let transport = auto_attach(transport, vec![attached("S2", "OOP")])
        .expect("Page.getFrameTree", oopif_frames)
        .expect("DOMSnapshot.captureSnapshot", json!({}))
        .expect("Accessibility.getFullAXTree", child_tree())
        .expect_error(
            "Accessibility.getFullAXTree",
            -32000,
            "Frame with the given id was not found.",
        )
        .expect("DOM.getFrameOwner", json!({ "backendNodeId": 30 }));
    let transport = auto_attach(transport, Vec::new())
        .expect("Target.setAutoAttach", json!({}))
        .expect("Target.setAutoAttach", json!({}));
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());

    let snapshot =
        snapshot_page(&mut connection, &session(), Deadline::after(1_000).unwrap()).unwrap();

    let preorder = snapshot.preorder();
    assert_eq!(preorder[2].0.role, "group");
    assert_eq!(preorder[3].0.identity.name.as_deref(), Some("Inner"));
    assert_eq!(
        preorder[4].0.identity.retained_object.as_deref(),
        Some("cdp:OOP:OOP:LO:101")
    );
    assert_eq!(snapshot.unreadable_frames, ["OOP-CHILD"]);
    let sent = sent.lock().unwrap();
    let oopif_calls: Vec<&str> = sent
        .iter()
        .filter(|request| request["sessionId"] == "S2")
        .map(|request| request["method"].as_str().unwrap())
        .collect();
    assert_eq!(
        oopif_calls,
        [
            "Page.getFrameTree",
            "DOMSnapshot.captureSnapshot",
            "Accessibility.getFullAXTree",
            "Accessibility.getFullAXTree",
            "Target.setAutoAttach",
            "Target.setAutoAttach",
        ]
    );
    let owner = sent
        .iter()
        .find(|request| request["method"] == "DOM.getFrameOwner")
        .unwrap();
    assert_eq!(owner["sessionId"], "S");
    assert_eq!(owner["params"]["frameId"], "OOP");
}

#[test]
fn an_iframe_target_that_vanishes_mid_read_is_listed_unreadable() {
    let transport = ScriptedTransport::new()
        .expect(
            "Page.getFrameTree",
            json!({ "frameTree": { "frame": { "id": "MAIN", "loaderId": "LM" } } }),
        )
        .expect("DOMSnapshot.captureSnapshot", json!({}))
        .expect("Accessibility.getFullAXTree", main_tree());
    let transport = auto_attach(transport, vec![attached("S2", "OOP")])
        .expect_error(
            "Page.getFrameTree",
            -32001,
            "Session with given id not found.",
        )
        .expect("Target.setAutoAttach", json!({}));
    let mut connection = CdpConnection::new(transport.boxed());

    let snapshot =
        snapshot_page(&mut connection, &session(), Deadline::after(1_000).unwrap()).unwrap();

    assert_eq!(snapshot.unreadable_frames, ["OOP"]);
    assert_eq!(snapshot.preorder().len(), 3);
}
