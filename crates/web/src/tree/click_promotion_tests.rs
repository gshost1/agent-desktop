use std::collections::HashSet;

use agent_desktop_core::NameSource;
use agent_desktop_core::refs::ref_alloc::is_ref_able;
use serde_json::{Value, json};

use crate::tree::cdp_ax_node::CdpAxNode;
use crate::tree::dom_facts::DomFacts;
use crate::tree::frame_build::build_frame;
use crate::tree::frame_context::FrameContext;
use crate::tree::web_node::WebNode;

fn build(raw: Value, clickable: &[i64]) -> WebNode {
    let dom = DomFacts {
        clickable: clickable.iter().copied().collect::<HashSet<_>>(),
        ..DomFacts::default()
    };
    let context = FrameContext {
        target_id: "T",
        frame_id: "F",
        loader_id: "L",
        dom: &dom,
    };
    let nodes: Vec<CdpAxNode> = serde_json::from_value(raw).unwrap();
    build_frame(nodes, &context).remove(0)
}

fn role(name: &str) -> Value {
    json!({ "type": "role", "value": name })
}

fn text(value: &str) -> Value {
    json!({ "type": "computedString", "value": value })
}

fn choice_list(rows: &[(&str, i64)]) -> Value {
    let mut nodes = vec![
        json!({ "nodeId": "1", "role": role("RootWebArea"), "name": text("Page"), "childIds": ["2"], "backendDOMNodeId": 1 }),
    ];
    let items: Vec<String> = rows
        .iter()
        .map(|(_, backend)| format!("li{backend}"))
        .collect();
    nodes.push(
        json!({ "nodeId": "2", "role": role("list"), "childIds": items, "backendDOMNodeId": 2 }),
    );
    for (label, backend) in rows {
        nodes.push(
            json!({ "nodeId": format!("li{backend}"), "role": role("listitem"),
            "childIds": [format!("span{backend}")], "backendDOMNodeId": backend * 10 }),
        );
        nodes.push(
            json!({ "nodeId": format!("span{backend}"), "role": role("generic"),
            "childIds": [format!("text{backend}")], "backendDOMNodeId": backend }),
        );
        nodes.push(
            json!({ "nodeId": format!("text{backend}"), "role": role("StaticText"),
            "name": text(label), "backendDOMNodeId": backend * 100 }),
        );
    }
    Value::Array(nodes)
}

fn preorder(node: &WebNode) -> Vec<&WebNode> {
    let mut all = vec![node];
    for child in &node.children {
        all.extend(preorder(child));
    }
    all
}

fn clickable_nodes(root: &WebNode) -> Vec<&WebNode> {
    preorder(root)
        .into_iter()
        .filter(|node| is_ref_able(&node.node))
        .collect()
}

#[test]
fn a_clickable_span_row_takes_a_click_its_token_and_its_text_as_a_derived_name() {
    let root = build(
        choice_list(&[("Springfield, IL", 4), ("Springfield, MO", 5)]),
        &[4, 5],
    );

    let rows = clickable_nodes(&root);
    assert_eq!(rows.len(), 2);
    let row = rows[0];
    assert_eq!(row.node.role, "group");
    assert_eq!(row.node.presentation.available_actions, ["Click"]);
    assert_eq!(
        row.node.identity.retained_object.as_deref(),
        Some("cdp:T:F:L:4")
    );
    assert_eq!(row.node.identity.name, None);
    let descriptors = &row.node.presentation.descriptors;
    assert_eq!(descriptors.name_source, Some(NameSource::ContainedText));
    assert_eq!(descriptors.derived_name.as_deref(), Some("Springfield, IL"));
    assert!(row.children.is_empty());
}

#[test]
fn without_the_click_fact_the_same_rows_read_as_bare_text() {
    let root = build(
        choice_list(&[("Springfield, IL", 4), ("Springfield, MO", 5)]),
        &[],
    );

    assert!(clickable_nodes(&root).is_empty());
    let roles: Vec<&str> = preorder(&root)
        .iter()
        .map(|node| node.node.role.as_str())
        .collect();
    assert_eq!(roles, ["webarea", "list", "statictext", "statictext"]);
}

#[test]
fn a_row_of_several_text_runs_is_named_by_their_text_in_reading_order_and_drops_them() {
    let raw = json!([
        { "nodeId": "1", "role": role("RootWebArea"), "childIds": ["2"], "backendDOMNodeId": 1 },
        { "nodeId": "2", "role": role("generic"), "childIds": ["3", "4", "5"], "backendDOMNodeId": 2 },
        { "nodeId": "3", "role": role("StaticText"), "name": text("Honolulu, HI"), "backendDOMNodeId": 3 },
        { "nodeId": "4", "role": role("StaticText"), "name": text(", USA "), "backendDOMNodeId": 4 },
        { "nodeId": "5", "role": role("StaticText"), "name": text(" (County)"), "backendDOMNodeId": 5 },
    ]);

    let root = build(raw, &[2]);

    let row = &root.children[0];
    assert_eq!(
        row.node.presentation.descriptors.derived_name.as_deref(),
        Some("Honolulu, HI, USA (County)")
    );
    assert!(row.children.is_empty());
}

#[test]
fn a_clickable_container_holding_a_control_stays_a_container() {
    let raw = json!([
        { "nodeId": "1", "role": role("RootWebArea"), "childIds": ["2"], "backendDOMNodeId": 1 },
        { "nodeId": "2", "role": role("generic"), "childIds": ["3", "5"], "backendDOMNodeId": 2 },
        { "nodeId": "3", "role": role("button"), "name": text("Buy"), "childIds": ["4"], "backendDOMNodeId": 3 },
        { "nodeId": "4", "role": role("StaticText"), "name": text("Buy"), "backendDOMNodeId": 4 },
        { "nodeId": "5", "role": role("StaticText"), "name": text("Card text"), "backendDOMNodeId": 5 },
    ]);

    let root = build(raw, &[1, 2]);

    let refs = clickable_nodes(&root);
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].node.role, "button");
}

#[test]
fn the_document_root_never_takes_a_click() {
    let raw = json!([
        { "nodeId": "1", "role": role("RootWebArea"), "childIds": ["2"], "backendDOMNodeId": 1 },
        { "nodeId": "2", "role": role("StaticText"), "name": text("Only text"), "backendDOMNodeId": 2 },
    ]);

    assert!(clickable_nodes(&build(raw, &[1, 2])).is_empty());
}

#[test]
fn a_named_node_keeps_its_accessible_name_and_a_control_keeps_its_actions() {
    let raw = json!([
        { "nodeId": "1", "role": role("RootWebArea"), "childIds": ["2", "4"], "backendDOMNodeId": 1 },
        { "nodeId": "2", "role": role("heading"), "name": text("Section"), "childIds": ["3"], "backendDOMNodeId": 2 },
        { "nodeId": "3", "role": role("StaticText"), "name": text("Section"), "backendDOMNodeId": 3 },
        { "nodeId": "4", "role": role("button"), "name": text("Go"), "backendDOMNodeId": 4,
          "properties": [{ "name": "focusable", "value": { "type": "booleanOrUndefined", "value": true } }] },
    ]);

    let root = build(raw, &[2, 4]);

    let heading = &root.children[0];
    assert_eq!(heading.node.presentation.available_actions, ["Click"]);
    assert_eq!(heading.node.identity.name.as_deref(), Some("Section"));
    assert_eq!(heading.node.presentation.descriptors.name_source, None);
    let button = &root.children[1];
    assert_eq!(
        button.node.presentation.available_actions,
        ["Click", "SetFocus"]
    );
}

#[test]
fn a_node_holding_more_text_than_a_name_keeps_is_a_region_and_takes_no_click() {
    let long = "word ".repeat(41);
    let root = build(choice_list(&[(long.as_str(), 4)]), &[4]);
    assert!(clickable_nodes(&root).is_empty());

    let fits = "word ".repeat(40);
    let root = build(choice_list(&[(fits.as_str(), 4)]), &[4]);
    let row = clickable_nodes(&root)[0];
    let name = row.node.presentation.descriptors.derived_name.as_deref();
    assert_eq!(name.map(|name| name.chars().count()), Some(199));
}

#[test]
fn a_clickable_wrapper_around_a_child_frame_takes_no_click() {
    let raw = json!([
        { "nodeId": "1", "role": role("RootWebArea"), "childIds": ["2"], "backendDOMNodeId": 1 },
        { "nodeId": "2", "role": role("generic"), "childIds": ["3"], "backendDOMNodeId": 2 },
        { "nodeId": "3", "role": role("generic"), "childIds": ["4"], "backendDOMNodeId": 3 },
        { "nodeId": "4", "role": role("Iframe"), "backendDOMNodeId": 4 },
    ]);

    assert!(clickable_nodes(&build(raw, &[2])).is_empty());
}

#[test]
fn live_regions_and_tab_panels_that_listen_for_clicks_take_none() {
    for container in ["tabpanel", "status", "log", "alert", "timer", "marquee"] {
        let raw = json!([
            { "nodeId": "1", "role": role("RootWebArea"), "childIds": ["2"], "backendDOMNodeId": 1 },
            { "nodeId": "2", "role": role(container), "childIds": ["3"], "backendDOMNodeId": 2 },
            { "nodeId": "3", "role": role("StaticText"), "name": text("Saved"), "backendDOMNodeId": 3 },
        ]);
        assert!(clickable_nodes(&build(raw, &[2])).is_empty(), "{container}");
    }
}

#[test]
fn a_list_that_delegates_clicks_to_its_items_takes_no_click() {
    let raw = json!([
        { "nodeId": "1", "role": role("RootWebArea"), "childIds": ["2"], "backendDOMNodeId": 1 },
        { "nodeId": "2", "role": role("list"), "childIds": ["3", "5"], "backendDOMNodeId": 2 },
        { "nodeId": "3", "role": role("listitem"), "childIds": ["4"], "backendDOMNodeId": 3 },
        { "nodeId": "4", "role": role("StaticText"), "name": text("First"), "backendDOMNodeId": 4 },
        { "nodeId": "5", "role": role("listitem"), "childIds": ["6"], "backendDOMNodeId": 5 },
        { "nodeId": "6", "role": role("StaticText"), "name": text("Second"), "backendDOMNodeId": 6 },
    ]);

    assert!(clickable_nodes(&build(raw, &[2])).is_empty());
}

#[test]
fn a_settable_date_input_that_listens_for_clicks_keeps_only_its_set_value() {
    let raw = json!([
        { "nodeId": "1", "role": role("RootWebArea"), "childIds": ["2"], "backendDOMNodeId": 1 },
        { "nodeId": "2", "role": role("date"), "name": text("Arrival"), "backendDOMNodeId": 2,
          "properties": [{ "name": "settable", "value": { "type": "booleanOrUndefined", "value": true } }] },
    ]);

    let root = build(raw, &[2]);

    let field = &root.children[0];
    assert_eq!(field.node.role, "datefield");
    assert_eq!(field.node.presentation.available_actions, ["SetValue"]);
}
