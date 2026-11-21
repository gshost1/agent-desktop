use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use agent_desktop_core::{AdapterError, Deadline, ErrorCode};
use serde_json::{Value, json};

use crate::transport::cdp_transport::CdpTransport;
use crate::transport::scripted_transport::ScriptedTransport;
use crate::transport::transport_error::transport_timeout;
use crate::tree::snapshot::{snapshot_page, snapshot_page_within_budget};
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
fn an_in_page_iframe_that_misses_its_budget_is_listed_and_the_page_returned() {
    let transport = ScriptedTransport::new()
        .expect("Page.getFrameTree", frame_tree())
        .expect("DOMSnapshot.captureSnapshot", capture())
        .expect("Accessibility.getFullAXTree", main_tree())
        .expect_silence("Accessibility.getFullAXTree")
        .expect("DOM.getFrameOwner", json!({}))
        .expect("Accessibility.getFullAXTree", child_tree())
        .expect_silence("DOM.getFrameOwner");
    let transport = auto_attach(transport, Vec::new()).expect("Target.setAutoAttach", json!({}));
    let mut connection = CdpConnection::new(transport.boxed());

    let snapshot =
        snapshot_page_within_budget(&mut connection, &session(), Deadline::after(1_000).unwrap())
            .unwrap();

    assert_eq!(snapshot.unreadable_frames, ["CHILD", "REMOTE"]);
    assert_eq!(snapshot.preorder().len(), 3);
}

#[test]
fn a_whole_deadline_snapshot_still_fails_on_a_slow_in_page_iframe() {
    let transport = ScriptedTransport::new()
        .expect("Page.getFrameTree", frame_tree())
        .expect("DOMSnapshot.captureSnapshot", capture())
        .expect("Accessibility.getFullAXTree", main_tree())
        .expect_silence("Accessibility.getFullAXTree")
        .expect("DOM.getFrameOwner", json!({}))
        .expect("Accessibility.getFullAXTree", child_tree())
        .expect("DOM.getFrameOwner", json!({}));
    let mut connection = CdpConnection::new(transport.boxed());

    let error =
        snapshot_page(&mut connection, &session(), Deadline::after(1_000).unwrap()).unwrap_err();

    assert_eq!(error.code, ErrorCode::Timeout);
}

type Summary = (String, Option<String>, Option<(String, i64)>);

#[derive(Default)]
struct ReorderingTransport {
    replies: HashMap<String, (Vec<Value>, Value)>,
    silent: HashSet<String>,
    held: Vec<String>,
    inbound: VecDeque<String>,
    most_held: Arc<Mutex<usize>>,
}

impl ReorderingTransport {
    fn answer(mut self, method: &str, session: &str, frame: &str, reply: Value) -> Self {
        let key = format!("{method}|{session}|{frame}");
        self.replies.insert(key, (Vec::new(), reply));
        self
    }

    fn silence(mut self, method: &str, session: &str) -> Self {
        self.silent.insert(format!("{method}|{session}|"));
        self
    }

    fn announce(mut self, session: &str, events: Vec<Value>) -> Self {
        let key = format!("Target.setAutoAttach|{session}|");
        self.replies.insert(key, (events, json!({})));
        self
    }
}

impl CdpTransport for ReorderingTransport {
    fn send_text(&mut self, text: &str, _deadline: Deadline) -> Result<(), AdapterError> {
        let request: Value = serde_json::from_str(text).unwrap();
        let key = format!(
            "{}|{}|{}",
            request["method"].as_str().unwrap(),
            request["sessionId"].as_str().unwrap_or_default(),
            request["params"]["frameId"].as_str().unwrap_or_default(),
        );
        let (events, result) = self
            .replies
            .get(&key)
            .cloned()
            .unwrap_or((Vec::new(), json!({})));
        self.inbound.extend(events.iter().map(Value::to_string));
        if self.silent.contains(&key) {
            return Ok(());
        }
        let reply =
            json!({ "id": request["id"], "sessionId": request["sessionId"], "result": result });
        self.held.push(reply.to_string());
        let mut most = self.most_held.lock().unwrap();
        *most = (*most).max(self.held.len());
        Ok(())
    }

    fn receive_text(&mut self, _deadline: Deadline) -> Result<String, AdapterError> {
        self.inbound
            .pop_front()
            .or_else(|| self.held.pop())
            .ok_or_else(|| transport_timeout("receive"))
    }
}

fn ax(nodes: &[(&str, &str, &str, i64)]) -> Value {
    let ids: Vec<String> = (2..=nodes.len()).map(|id| id.to_string()).collect();
    let mut list = Vec::new();
    for (index, (kind, name, _, backend)) in nodes.iter().enumerate() {
        let mut node = json!({ "nodeId": (index + 1).to_string(), "role": role(kind), "backendDOMNodeId": backend });
        if !name.is_empty() {
            node["name"] = text(name);
        }
        if index == 0 {
            node["childIds"] = json!(ids);
        }
        list.push(node);
    }
    json!({ "nodes": list })
}

fn oopif(parent: &str, session: &str, target: &str, parent_frame: &str) -> Value {
    json!({ "method": "Target.attachedToTarget", "sessionId": parent, "params": {
        "sessionId": session, "waitingForDebugger": false,
        "targetInfo": { "targetId": target, "type": "iframe", "parentFrameId": parent_frame },
    } })
}

fn oopif_frames(frame: &str, parent_frame: &str) -> Value {
    json!({ "frameTree": { "frame": { "id": frame, "parentId": parent_frame, "loaderId": "L" } } })
}

#[test]
fn frames_read_concurrently_keep_the_sequential_content_and_depth_first_order() {
    let main = ax(&[
        ("RootWebArea", "Top", "", 1),
        ("button", "Go", "", 20),
        ("Iframe", "", "", 30),
        ("Iframe", "", "", 40),
        ("Iframe", "", "", 50),
        ("Iframe", "", "", 60),
    ]);
    let alpha = ax(&[
        ("RootWebArea", "Alpha", "", 200),
        ("button", "A1", "", 201),
        ("Iframe", "", "", 202),
    ]);
    let beta = ax(&[("RootWebArea", "Beta", "", 300), ("link", "B1", "", 301)]);
    let gamma = ax(&[("RootWebArea", "Gamma", "", 400), ("link", "C1", "", 401)]);
    let transport = ReorderingTransport::default()
        .answer("Page.getFrameTree", "S", "", frame_tree())
        .answer("Accessibility.getFullAXTree", "S", "MAIN", main)
        .answer("Accessibility.getFullAXTree", "S", "CHILD", child_tree())
        .answer(
            "DOM.getFrameOwner",
            "S",
            "CHILD",
            json!({ "backendNodeId": 30 }),
        )
        .answer(
            "Accessibility.getFullAXTree",
            "S",
            "REMOTE",
            json!({ "nodes": [] }),
        )
        .announce(
            "S",
            vec![
                oopif("S", "S2", "A", "MAIN"),
                oopif("S", "S3", "B", "MAIN"),
                oopif("S", "S5", "D", "MAIN"),
            ],
        )
        .announce("S2", vec![oopif("S2", "S4", "C", "A")])
        .answer("Page.getFrameTree", "S2", "", oopif_frames("A", "MAIN"))
        .answer("Page.getFrameTree", "S3", "", oopif_frames("B", "MAIN"))
        .answer("Page.getFrameTree", "S4", "", oopif_frames("C", "A"))
        .silence("Page.getFrameTree", "S5")
        .answer("Accessibility.getFullAXTree", "S2", "A", alpha)
        .answer("Accessibility.getFullAXTree", "S3", "B", beta)
        .answer("Accessibility.getFullAXTree", "S4", "C", gamma)
        .answer(
            "DOM.getFrameOwner",
            "S",
            "A",
            json!({ "backendNodeId": 40 }),
        )
        .answer(
            "DOM.getFrameOwner",
            "S",
            "B",
            json!({ "backendNodeId": 50 }),
        )
        .answer(
            "DOM.getFrameOwner",
            "S2",
            "C",
            json!({ "backendNodeId": 202 }),
        );
    let most_held = Arc::clone(&transport.most_held);
    let mut connection = CdpConnection::new(Box::new(transport));

    let snapshot =
        snapshot_page_within_budget(&mut connection, &session(), Deadline::after(1_000).unwrap())
            .unwrap();

    let summary: Vec<Summary> = snapshot
        .preorder()
        .iter()
        .map(|(node, origin)| {
            (
                node.role.clone(),
                node.identity.name.clone(),
                origin.map(|origin| (origin.frame_id.clone(), origin.backend_node_id)),
            )
        })
        .collect();
    let expected = [
        ("webarea", Some("Top"), "MAIN", 1),
        ("button", Some("Go"), "MAIN", 20),
        ("group", None, "MAIN", 30),
        ("webarea", Some("Inner"), "CHILD", 100),
        ("link", Some("More"), "CHILD", 101),
        ("group", None, "MAIN", 40),
        ("webarea", Some("Alpha"), "A", 200),
        ("button", Some("A1"), "A", 201),
        ("group", None, "A", 202),
        ("webarea", Some("Gamma"), "C", 400),
        ("link", Some("C1"), "C", 401),
        ("group", None, "MAIN", 50),
        ("webarea", Some("Beta"), "B", 300),
        ("link", Some("B1"), "B", 301),
        ("group", None, "MAIN", 60),
    ]
    .map(|(role, name, frame, backend)| {
        (
            role.to_owned(),
            name.map(str::to_owned),
            Some((frame.to_owned(), backend)),
        )
    });
    assert_eq!(summary, expected);
    assert_eq!(snapshot.unreadable_frames, ["D"]);
    assert!(*most_held.lock().unwrap() > 1);
}
