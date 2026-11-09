use std::collections::HashSet;

use agent_desktop_core::NameSource;
use serde_json::{Value, json};

use crate::tree::cdp_ax_node::CdpAxNode;
use crate::tree::dom_facts::DomFacts;
use crate::tree::frame_build::build_frame;
use crate::tree::frame_context::FrameContext;
use crate::tree::web_node::WebNode;

fn build(raw: Value, inline: &[i64], clickable: &[i64]) -> WebNode {
    let dom = DomFacts {
        inline: inline.iter().copied().collect::<HashSet<_>>(),
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

fn node(id: u32, role: &str, name: Option<&str>, children: &[u32]) -> Value {
    let mut node = json!({
        "nodeId": id.to_string(),
        "role": { "type": "role", "value": role },
        "childIds": children.iter().map(u32::to_string).collect::<Vec<_>>(),
        "backendDOMNodeId": id,
    });
    if let Some(name) = name {
        node["name"] = json!({ "type": "computedString", "value": name });
    }
    if role == "link" || role == "textbox" {
        node["properties"] = json!([{ "name": "focusable", "value": { "type": "booleanOrUndefined", "value": true } }]);
    }
    node
}

fn page(body: &[u32], rest: Vec<Value>) -> Value {
    let mut nodes = vec![node(1, "RootWebArea", Some("Page"), body)];
    nodes.extend(rest);
    Value::Array(nodes)
}

fn summary(node: &WebNode) -> Vec<String> {
    let mut lines = Vec::new();
    describe(node, 0, &mut lines);
    lines
}

fn describe(node: &WebNode, depth: usize, lines: &mut Vec<String>) {
    let descriptors = &node.node.presentation.descriptors;
    let name = node
        .node
        .identity
        .name
        .clone()
        .or_else(|| {
            descriptors
                .derived_name
                .clone()
                .map(|text| format!("~{text}"))
        })
        .unwrap_or_default();
    lines.push(format!("{}{} {name}", "  ".repeat(depth), node.node.role));
    for child in &node.children {
        describe(child, depth + 1, lines);
    }
}

fn paragraph_with_footnote() -> Value {
    page(
        &[10],
        vec![
            node(10, "paragraph", None, &[11, 12, 13, 14, 20]),
            node(11, "StaticText", Some("Conceived in the late 1980s"), &[]),
            node(12, "superscript", None, &[15]),
            node(15, "link", Some("[1]"), &[16, 17, 18]),
            node(16, "StaticText", Some("["), &[]),
            node(17, "StaticText", Some("1"), &[]),
            node(18, "StaticText", Some("]"), &[]),
            node(13, "StaticText", Some(" by "), &[]),
            node(14, "link", Some("Ada Example"), &[19]),
            node(19, "StaticText", Some("Ada Example"), &[]),
            node(20, "StaticText", Some(", at the institute."), &[]),
        ],
    )
}

#[test]
fn a_paragraph_reads_as_one_derived_name_and_keeps_its_links() {
    let root = build(paragraph_with_footnote(), &[12, 15, 14], &[]);

    assert_eq!(
        summary(&root),
        [
            "webarea Page",
            "  paragraph ~Conceived in the late 1980s[1] by Ada Example, at the institute.",
            "    link [1]",
            "    link Ada Example",
        ]
    );
    let paragraph = &root.children[0].node;
    assert_eq!(paragraph.identity.name, None);
    assert_eq!(
        paragraph.presentation.descriptors.name_source,
        Some(NameSource::ContainedText)
    );
    let links: Vec<_> = root.children[0]
        .children
        .iter()
        .map(|link| link.node.identity.retained_object.clone())
        .collect();
    assert_eq!(
        links,
        [
            Some("cdp:T:F:L:15".to_owned()),
            Some("cdp:T:F:L:14".to_owned())
        ]
    );
}

#[test]
fn text_never_merges_across_the_edge_of_a_block_level_element() {
    let raw = page(
        &[10],
        vec![
            node(10, "generic", None, &[11, 12, 13]),
            node(11, "generic", None, &[21]),
            node(21, "StaticText", Some("Title A"), &[]),
            node(12, "generic", None, &[22]),
            node(22, "StaticText", Some("Subtitle B"), &[]),
            node(13, "generic", None, &[23, 24]),
            node(23, "StaticText", Some("Span "), &[]),
            node(24, "StaticText", Some("tail"), &[]),
        ],
    );
    let root = build(raw, &[13], &[]);

    assert_eq!(
        summary(&root),
        [
            "webarea Page",
            "  statictext Title A",
            "  statictext Subtitle B",
            "  statictext Span tail",
        ]
    );
}

#[test]
fn a_list_item_with_a_link_keeps_its_own_node_and_one_without_reads_as_one_run() {
    let raw = page(
        &[10],
        vec![
            node(10, "list", None, &[11, 12, 13]),
            node(11, "listitem", None, &[21, 22]),
            node(21, "StaticText", Some("First item with "), &[]),
            node(22, "emphasis", None, &[23]),
            node(23, "StaticText", Some("emphasis"), &[]),
            node(12, "listitem", None, &[24]),
            node(24, "link", Some("Only a link"), &[25]),
            node(25, "StaticText", Some("Only a link"), &[]),
            node(13, "listitem", None, &[26, 27, 28]),
            node(26, "StaticText", Some("Contact: "), &[]),
            node(27, "link", Some("the team"), &[29]),
            node(29, "StaticText", Some("the team"), &[]),
            node(28, "StaticText", Some(" or call."), &[]),
        ],
    );
    let root = build(raw, &[22, 24, 27], &[]);

    assert_eq!(
        summary(&root),
        [
            "webarea Page",
            "  list ",
            "    statictext First item with emphasis",
            "    link Only a link",
            "    group ~Contact: the team or call.",
            "      link the team",
        ]
    );
}

#[test]
fn a_block_holding_a_field_keeps_its_text_beside_the_field() {
    let raw = page(
        &[10],
        vec![
            node(10, "paragraph", None, &[11, 12, 13]),
            node(11, "StaticText", Some("Full"), &[]),
            node(12, "StaticText", Some(" name"), &[]),
            node(13, "textbox", Some("Full name"), &[]),
        ],
    );
    let root = build(raw, &[], &[]);

    assert_eq!(
        summary(&root),
        [
            "webarea Page",
            "  paragraph ",
            "    statictext Full name",
            "    textfield Full name",
        ]
    );
}

#[test]
fn a_line_break_reads_as_a_space_and_blank_runs_leave_no_node() {
    let raw = page(
        &[10],
        vec![
            node(10, "paragraph", None, &[11, 12, 13]),
            node(11, "StaticText", Some("Line one"), &[]),
            node(12, "LineBreak", Some("\n"), &[]),
            node(13, "StaticText", Some("Line two"), &[]),
            node(14, "StaticText", Some(" "), &[]),
        ],
    );
    let mut raw = raw;
    raw[1]["childIds"] = json!(["14", "11", "12", "13"]);
    let root = build(raw, &[], &[]);

    assert_eq!(
        summary(&root),
        ["webarea Page", "  paragraph ~Line one Line two"]
    );
}

#[test]
fn a_named_cell_drops_the_runs_its_name_already_holds() {
    let raw = page(
        &[10],
        vec![
            node(10, "cell", Some("Version 3.0 final"), &[11, 12, 13]),
            node(11, "StaticText", Some("Version "), &[]),
            node(12, "link", Some("3.0"), &[14]),
            node(14, "StaticText", Some("3.0"), &[]),
            node(13, "StaticText", Some(" final"), &[]),
        ],
    );
    let root = build(raw, &[12], &[]);

    assert_eq!(
        summary(&root),
        ["webarea Page", "  cell Version 3.0 final", "    link 3.0"]
    );
}

#[test]
fn a_clickable_element_named_by_its_text_drops_the_runs_it_names() {
    let raw = page(
        &[10],
        vec![
            node(10, "generic", None, &[11, 12, 13]),
            node(11, "StaticText", Some("Clickable "), &[]),
            node(12, "StaticText", Some("row"), &[]),
            node(13, "StaticText", Some(" text"), &[]),
        ],
    );
    let root = build(raw, &[], &[10]);

    assert_eq!(
        summary(&root),
        ["webarea Page", "  group ~Clickable row text"]
    );
    assert!(root.children[0].node.identity.retained_object.is_some());
}

#[test]
fn a_clickable_card_is_named_by_the_text_of_the_blocks_it_holds() {
    let raw = page(
        &[10],
        vec![
            node(10, "generic", None, &[11, 12]),
            node(11, "paragraph", None, &[21]),
            node(21, "StaticText", Some("Springfield"), &[]),
            node(12, "paragraph", None, &[22, 23]),
            node(22, "StaticText", Some("Population "), &[]),
            node(23, "StaticText", Some("167,000"), &[]),
        ],
    );
    let root = build(raw, &[], &[10]);

    let card = &root.children[0].node;
    assert_eq!(
        card.presentation.descriptors.derived_name.as_deref(),
        Some("Springfield Population 167,000")
    );
}

#[test]
fn a_clickable_card_keeps_the_words_of_spliced_blocks_apart() {
    let raw = page(
        &[10],
        vec![
            node(10, "generic", None, &[11, 12]),
            node(11, "generic", None, &[21]),
            node(21, "StaticText", Some("Title A"), &[]),
            node(12, "generic", None, &[22]),
            node(22, "StaticText", Some("Subtitle B"), &[]),
        ],
    );
    let root = build(raw, &[], &[10]);

    let card = &root.children[0].node;
    assert_eq!(
        card.presentation.descriptors.derived_name.as_deref(),
        Some("Title A Subtitle B")
    );
}

#[test]
fn an_inline_wrapper_with_only_a_dom_id_is_read_through_and_a_block_one_is_kept() {
    let raw = page(
        &[10, 30],
        vec![
            node(10, "paragraph", None, &[11, 12, 13]),
            node(11, "generic", None, &[21]),
            node(21, "StaticText", Some("Python "), &[]),
            node(12, "superscript", None, &[22]),
            node(22, "link", Some("[11]"), &[23]),
            node(23, "StaticText", Some("[11]"), &[]),
            node(13, "StaticText", Some(" is a language."), &[]),
            node(30, "generic", None, &[31]),
            node(31, "StaticText", Some("Hatnote"), &[]),
        ],
    );
    let ids = [11, 12, 30, 22]
        .into_iter()
        .map(|backend| (backend, format!("id{backend}")))
        .collect();
    let dom = DomFacts {
        ids,
        inline: HashSet::from([11, 12, 22]),
        ..DomFacts::default()
    };
    let context = FrameContext {
        target_id: "T",
        frame_id: "F",
        loader_id: "L",
        dom: &dom,
    };
    let nodes: Vec<CdpAxNode> = serde_json::from_value(raw).unwrap();
    let root = build_frame(nodes, &context).remove(0);

    assert_eq!(
        summary(&root),
        [
            "webarea Page",
            "  paragraph ~Python [11] is a language.",
            "    link [11]",
            "  group ~Hatnote",
        ]
    );
    let link = &root.children[0].children[0].node;
    assert_eq!(
        link.identity.native_id.as_ref().map(|id| id.value.as_str()),
        Some("id22")
    );
    let hatnote = &root.children[1].node;
    assert_eq!(
        hatnote
            .identity
            .native_id
            .as_ref()
            .map(|id| id.value.as_str()),
        Some("id30")
    );
}
