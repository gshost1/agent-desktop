use super::summarize;
use crate::{AccessibilityNode, adapter::minimal_accessibility_node, snapshot::ref_lines};

fn node(role: &str, name: &str) -> AccessibilityNode {
    minimal_accessibility_node(role, name)
}

fn with_ref(mut node: AccessibilityNode, ref_id: &str) -> AccessibilityNode {
    node.ref_id = Some(ref_id.into());
    node
}

fn parent(mut node: AccessibilityNode, children: Vec<AccessibilityNode>) -> AccessibilityNode {
    node.children = children;
    node
}

fn buttons(count: usize, prefix: &str) -> Vec<AccessibilityNode> {
    (0..count)
        .map(|i| {
            with_ref(
                node("button", &format!("{prefix} {i}")),
                &format!("@s1:{prefix}{i}"),
            )
        })
        .collect()
}

fn scrollbar(enabled: bool) -> AccessibilityNode {
    let mut bar = node("scrollbar", "");
    if !enabled {
        bar.presentation.states = vec!["disabled".into()];
    }
    bar
}

fn window(children: Vec<AccessibilityNode>) -> AccessibilityNode {
    parent(with_ref(node("window", "Main"), "@s1:e0"), children)
}

fn lazy_list(rows: usize, enabled_bar: bool) -> AccessibilityNode {
    parent(
        with_ref(node("scrollarea", "Rows"), "@s1:area"),
        vec![
            parent(node("group", ""), buttons(rows, "row")),
            scrollbar(enabled_bar),
        ],
    )
}

#[test]
fn a_tree_small_enough_to_read_whole_gets_no_contents() {
    assert_eq!(summarize(&window(buttons(10, "b"))), None);
}

#[test]
fn a_scrolling_list_reports_its_loaded_items_and_that_more_may_exist() {
    let toolbar = parent(node("toolbar", ""), buttons(3, "tool"));
    let tree = window(vec![toolbar, lazy_list(45, true)]);
    let contents = summarize(&tree).expect("contents");
    assert_eq!(contents.roles.get("button"), Some(&48));
    let regions = &contents.regions;
    assert_eq!(regions.len(), 2);
    assert_eq!((regions[0].role.as_str(), regions[0].line), ("toolbar", 1));
    assert_eq!(regions[0].items, None);
    assert_eq!(regions[1].role, "scrollarea");
    assert_eq!(regions[1].ref_id.as_deref(), Some("@s1:area"));
    assert_eq!(regions[1].items, Some(45));
    assert!(regions[1].partial);
}

#[test]
fn a_region_line_is_its_index_in_the_lines_output() {
    let tree = window(vec![
        parent(node("group", ""), buttons(20, "a")),
        lazy_list(30, true),
    ]);
    let lines = ref_lines::render_ref_lines(&tree);
    let contents = summarize(&tree).expect("contents");
    let region = &contents.regions[0];
    assert!(
        lines[region.line].contains("@s1:area scrollarea"),
        "{}",
        lines[region.line]
    );
}

#[test]
fn a_list_in_a_scroll_area_is_the_region_and_a_disabled_bar_is_not_partial() {
    let rows = (0..50).map(|i| with_ref(node("row", &format!("r{i}")), &format!("@s1:r{i}")));
    let outline = parent(
        with_ref(node("outline", "Files"), "@s1:list"),
        std::iter::once(node("column", "Name"))
            .chain(rows)
            .collect(),
    );
    let area = parent(node("scrollarea", ""), vec![outline, scrollbar(false)]);
    let contents = summarize(&window(vec![area])).expect("contents");
    assert_eq!(contents.regions.len(), 1);
    let region = &contents.regions[0];
    assert_eq!((region.role.as_str(), region.items), ("outline", Some(50)));
    assert!(!region.partial);
}

#[test]
fn the_index_keeps_the_commonest_roles_and_the_first_regions() {
    let roles = [
        "button",
        "cell",
        "checkbox",
        "link",
        "menuitem",
        "radiobutton",
        "tab",
    ];
    let mut children: Vec<AccessibilityNode> = roles
        .iter()
        .enumerate()
        .flat_map(|(rank, role)| {
            (0..(10 - rank)).map(move |i| with_ref(node(role, ""), &format!("@s1:{role}{i}")))
        })
        .collect();
    children.extend((0..8).map(|i| parent(node("list", &format!("L{i}")), buttons(1, "x"))));
    let contents = summarize(&window(children)).expect("contents");
    assert_eq!(contents.roles.len(), 4);
    assert!(!contents.roles.contains_key("menuitem"));
    assert!(!contents.roles.contains_key("window"));
    assert_eq!(contents.regions.len(), 6);
    assert_eq!(contents.more_regions, 2);
}

#[test]
fn a_long_region_name_is_shortened() {
    let long = "A list whose accessible name runs on far past what an index needs";
    let tree = window(vec![parent(node("list", long), buttons(45, "i"))]);
    let name = summarize(&tree).expect("contents").regions[0]
        .name
        .clone()
        .unwrap();
    assert_eq!(name.chars().count(), 32);
    assert!(name.ends_with('…'));
}

#[test]
fn contents_serialize_as_the_golden_file_shows() {
    let toolbar = parent(node("toolbar", ""), buttons(2, "tool"));
    let tree = window(vec![toolbar, lazy_list(40, true)]);
    let contents = serde_json::to_string_pretty(&summarize(&tree).expect("contents")).unwrap();
    let golden = include_str!("../../../../tests/fixtures/snapshot-contents.json");
    assert_eq!(contents, golden.replace("\r\n", "\n").trim_end());
}

#[test]
fn a_scroll_area_of_mixed_controls_is_not_a_list() {
    let mut mixed = buttons(20, "b");
    mixed.extend((0..20).map(|i| with_ref(node("checkbox", ""), &format!("@s1:c{i}"))));
    let page = parent(
        node("scrollarea", "Page"),
        vec![parent(node("group", ""), mixed)],
    );
    let contents = summarize(&window(vec![page])).expect("contents");
    assert!(contents.regions.is_empty());
}

#[test]
fn the_lines_form_puts_each_region_on_one_line_led_by_its_line() {
    let toolbar = parent(node("toolbar", ""), buttons(2, "tool"));
    let tree = window(vec![toolbar, lazy_list(40, true)]);
    let mut data = serde_json::json!({
        "contents": summarize(&tree).expect("contents"),
        "tree": tree,
    });
    assert!(ref_lines::replace_tree_with_lines(&mut data).unwrap());
    assert_eq!(
        data["contents"],
        serde_json::json!([
            "roles: button 42",
            "line 1: toolbar",
            "line 4: @s1:area scrollarea \"Rows\" items=40 partial",
        ])
    );
    assert!(data.get("tree").is_none());
}
