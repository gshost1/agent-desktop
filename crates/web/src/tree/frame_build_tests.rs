use std::collections::HashMap;

use serde_json::{Value, json};

use super::build_frame;
use crate::ElementToken;
use crate::tree::cdp_ax_node::CdpAxNode;
use crate::tree::dom_facts::DomFacts;
use crate::tree::frame_context::FrameContext;
use crate::tree::web_node::WebNode;

fn nodes(raw: Value) -> Vec<CdpAxNode> {
    serde_json::from_value(raw).unwrap()
}

fn build(raw: Value, dom_ids: &HashMap<i64, String>) -> Vec<WebNode> {
    let dom = DomFacts {
        ids: dom_ids.clone(),
        ..DomFacts::default()
    };
    let context = FrameContext {
        target_id: "T1",
        frame_id: "F1",
        loader_id: "L1",
        dom: &dom,
    };
    build_frame(nodes(raw), &context)
}

fn role(name: &str) -> Value {
    json!({ "type": "role", "value": name })
}

fn text(value: &str) -> Value {
    json!({ "type": "computedString", "value": value })
}

fn sample() -> Value {
    json!([
        { "nodeId": "1", "role": role("RootWebArea"), "name": text("Page"), "childIds": ["2"], "backendDOMNodeId": 1 },
        { "nodeId": "2", "ignored": true, "role": role("none"), "childIds": ["3"], "backendDOMNodeId": 2 },
        { "nodeId": "3", "role": role("generic"), "childIds": ["4", "6", "8"], "backendDOMNodeId": 3 },
        { "nodeId": "4", "role": role("button"), "name": text("Save"), "childIds": ["5"], "backendDOMNodeId": 4,
          "properties": [{ "name": "focusable", "value": { "type": "booleanOrUndefined", "value": true } }] },
        { "nodeId": "5", "role": role("StaticText"), "name": text("Save"), "childIds": ["9"], "backendDOMNodeId": 5 },
        { "nodeId": "9", "role": role("InlineTextBox"), "name": text("Save") },
        { "nodeId": "6", "role": role("heading"), "name": text("Title"), "childIds": ["7"], "backendDOMNodeId": 6 },
        { "nodeId": "7", "role": role("StaticText"), "name": text("Title"), "backendDOMNodeId": 7 },
        { "nodeId": "8", "role": role("generic"), "backendDOMNodeId": 8, "childIds": ["10"] },
        { "nodeId": "10", "role": role("textbox"), "name": text("Email"), "value": { "type": "string", "value": "a@b.c" },
          "backendDOMNodeId": 10, "childIds": ["11"] },
        { "nodeId": "11", "role": role("generic"), "backendDOMNodeId": 11,
          "properties": [{ "name": "editable", "value": { "type": "token", "value": "plaintext" } }] },
    ])
}

#[test]
fn ignored_nodes_and_empty_wrappers_are_spliced_out() {
    let roots = build(sample(), &HashMap::new());
    assert_eq!(roots.len(), 1);
    let root = &roots[0];
    assert_eq!(root.node.role, "webarea");
    assert_eq!(root.node.identity.name.as_deref(), Some("Page"));
    let roles: Vec<&str> = root
        .children
        .iter()
        .map(|child| child.node.role.as_str())
        .collect();
    assert_eq!(roles, ["button", "heading", "textfield"]);
}

#[test]
fn name_repeating_text_and_fragments_are_dropped() {
    let roots = build(sample(), &HashMap::new());
    let button = &roots[0].children[0];
    assert!(button.children.is_empty());
    let heading = &roots[0].children[1];
    assert!(heading.children.is_empty());
    let field = &roots[0].children[2];
    assert_eq!(field.node.identity.value.as_deref(), Some("a@b.c"));
    assert!(field.children.is_empty());
}

#[test]
fn interactive_nodes_carry_their_element_token_and_origin() {
    let roots = build(sample(), &HashMap::new());
    let button = &roots[0].children[0];
    let token = button.node.identity.retained_object.as_deref().unwrap();
    assert_eq!(
        ElementToken::parse(token).unwrap(),
        ElementToken {
            target_id: "T1".into(),
            frame_id: "F1".into(),
            loader_id: "L1".into(),
            backend_node_id: 4
        }
    );
    let origin = button.origin.as_ref().unwrap();
    assert_eq!(origin.frame_id, "F1");
    assert_eq!(origin.backend_node_id, 4);
    assert!(roots[0].children[1].node.identity.retained_object.is_none());
}

#[test]
fn a_dom_id_becomes_the_native_id_and_keeps_a_wrapper() {
    let dom_ids = HashMap::from([(4, "save-button".to_owned()), (8, "panel".to_owned())]);
    let roots = build(sample(), &dom_ids);
    let button = &roots[0].children[0];
    let native = button.node.identity.native_id.as_ref().unwrap();
    assert_eq!(native.value, "save-button");
    assert_eq!(
        native.kind,
        agent_desktop_core::IdentifierKind::AxDomIdentifier
    );
    let roles: Vec<&str> = roots[0]
        .children
        .iter()
        .map(|child| child.node.role.as_str())
        .collect();
    assert_eq!(roles, ["button", "heading", "group"]);
}

#[test]
fn cycles_and_missing_children_do_not_loop() {
    let raw = json!([
        { "nodeId": "1", "role": role("RootWebArea"), "childIds": ["2", "404"] },
        { "nodeId": "2", "role": role("link"), "name": text("Home"), "childIds": ["3"] },
        { "nodeId": "3", "role": role("group"), "name": text("Loop"), "childIds": ["2"] },
    ]);
    let roots = build(raw, &HashMap::new());
    let link = &roots[0].children[0];
    assert_eq!(link.node.role, "link");
    assert_eq!(link.children[0].node.identity.name.as_deref(), Some("Loop"));
    assert!(link.children[0].children.is_empty());
}

#[test]
fn an_empty_tree_builds_nothing() {
    assert!(build(json!([]), &HashMap::new()).is_empty());
}

#[test]
fn a_wrapper_that_only_takes_focus_is_still_transparent() {
    let raw = json!([
        { "nodeId": "1", "role": role("RootWebArea"), "childIds": ["2"] },
        { "nodeId": "2", "role": role("generic"), "childIds": ["3"], "backendDOMNodeId": 2,
          "properties": [{ "name": "focusable", "value": { "type": "booleanOrUndefined", "value": true } }] },
        { "nodeId": "3", "role": role("link"), "name": text("Docs"), "backendDOMNodeId": 3 },
    ]);
    let roots = build(raw, &HashMap::new());
    assert_eq!(roots[0].children[0].node.role, "link");
}
