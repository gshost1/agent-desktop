use super::summarize;
use crate::{AccessibilityNode, adapter::minimal_accessibility_node, snapshot::ref_lines};

pub(super) fn node(role: &str, name: &str) -> AccessibilityNode {
    minimal_accessibility_node(role, name)
}

fn with_ref(mut node: AccessibilityNode, ref_id: &str) -> AccessibilityNode {
    node.ref_id = Some(ref_id.into());
    node
}

pub(super) fn parent(
    mut node: AccessibilityNode,
    children: Vec<AccessibilityNode>,
) -> AccessibilityNode {
    node.children = children;
    node
}

pub(super) fn buttons(count: usize, prefix: &str) -> Vec<AccessibilityNode> {
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

pub(super) fn window(children: Vec<AccessibilityNode>) -> AccessibilityNode {
    parent(with_ref(node("window", "Main"), "@s1:e0"), children)
}

fn lazy_list(rows: usize, enabled_bar: bool) -> AccessibilityNode {
    let mut area = parent(
        with_ref(node("scrollarea", "Rows"), "@s1:area"),
        vec![
            parent(node("group", ""), buttons(rows, "row")),
            scrollbar(enabled_bar),
        ],
    );
    area.presentation.scrolls_vertically = Some(enabled_bar);
    area
}

#[test]
fn a_tree_small_enough_to_read_whole_gets_no_contents() {
    assert_eq!(summarize(&window(buttons(10, "b"))), None);
}

#[test]
fn a_scrolling_list_reports_its_loaded_items_and_that_more_may_exist() {
    let toolbar = parent(node("toolbar", ""), buttons(3, "tool"));
    let tree = window(vec![toolbar, lazy_list(85, true)]);
    let contents = summarize(&tree).expect("contents");
    assert_eq!(contents.roles.get("button"), Some(&88));
    let mut ordered = contents.regions.clone();
    ordered.sort_by_key(|region| region.line);
    let regions = &ordered;
    assert_eq!(regions.len(), 2);
    assert_eq!((regions[0].role.as_str(), regions[0].line), ("toolbar", 1));
    assert_eq!(regions[0].items, None);
    assert_eq!(regions[1].role, "scrollarea");
    assert_eq!(regions[1].ref_id.as_deref(), Some("@s1:area"));
    assert_eq!(regions[1].items, Some(85));
    assert!(regions[1].partial);
}

#[test]
fn a_region_line_is_its_index_in_the_lines_output() {
    let tree = window(vec![
        parent(node("group", ""), buttons(40, "a")),
        lazy_list(50, true),
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
    let rows = (0..90).map(|i| with_ref(node("row", &format!("r{i}")), &format!("@s1:r{i}")));
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
    assert_eq!((region.role.as_str(), region.items), ("outline", Some(90)));
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
    children.extend((0..8).map(|i| parent(node("list", &format!("L{i}")), buttons(3, "x"))));
    let contents = summarize(&window(children)).expect("contents");
    assert_eq!(contents.roles.len(), 4);
    assert!(!contents.roles.contains_key("menuitem"));
    assert!(!contents.roles.contains_key("window"));
    assert_eq!(contents.regions.len(), 6);
    assert_eq!(contents.more_regions, 2);
    assert_eq!(
        contents.lines().last().map(String::as_str),
        Some("+2 more regions")
    );
}

#[test]
fn a_long_region_name_is_shortened() {
    let long = "A list whose accessible name runs on far past what an index needs";
    let tree = window(vec![parent(node("list", long), buttons(85, "i"))]);
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
    let tree = window(vec![toolbar, lazy_list(80, true)]);
    let contents = serde_json::to_string_pretty(&summarize(&tree).expect("contents")).unwrap();
    let golden = include_str!("../../../../tests/fixtures/snapshot-contents.json");
    assert_eq!(contents, golden.replace("\r\n", "\n").trim_end());
}

#[test]
fn a_scroll_area_of_mixed_controls_is_not_a_list() {
    let mut mixed = buttons(40, "b");
    mixed.extend((0..40).map(|i| with_ref(node("checkbox", ""), &format!("@s1:c{i}"))));
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
    let tree = window(vec![toolbar, lazy_list(80, true)]);
    let mut data = serde_json::json!({
        "contents": summarize(&tree).expect("contents"),
        "tree": tree,
    });
    assert!(ref_lines::replace_tree_with_lines(&mut data).unwrap());
    assert_eq!(
        data["contents"],
        serde_json::json!([
            "roles: button 82",
            "line 4: @s1:area scrollarea \"Rows\" items=80 partial",
            "line 1: toolbar",
        ])
    );
    assert!(data.get("tree").is_none());
}

pub(super) fn filler() -> AccessibilityNode {
    parent(node("group", ""), buttons(85, "f"))
}

fn rows(role: &str, count: usize) -> Vec<AccessibilityNode> {
    (0..count)
        .map(|i| with_ref(node(role, &format!("item {i}")), &format!("@s1:{role}{i}")))
        .collect()
}

pub(super) fn only_region(
    tree: &AccessibilityNode,
) -> crate::snapshot::content_region::ContentRegion {
    let contents = summarize(tree).expect("contents");
    assert_eq!(contents.regions.len(), 1, "{:?}", contents.regions);
    contents.regions[0].clone()
}

#[test]
fn a_depth_cut_list_reports_the_count_its_line_prints_and_that_it_was_cut() {
    let mut outline = with_ref(node("outline", "Files"), "@s1:list");
    outline.children_count = Some(600);
    let region = only_region(&window(vec![filler(), outline]));
    assert_eq!(region.items, Some(600));
    assert!(region.partial);
}

#[test]
fn a_list_the_refmap_cap_cut_is_partial_after_its_scroll_bar_was_cut_away() {
    let mut outline = parent(
        with_ref(node("outline", "Files"), "@s1:list"),
        rows("row", 85),
    );
    outline.subtree_truncated = true;
    let area = parent(node("scrollarea", ""), vec![outline]);
    let region = only_region(&window(vec![area]));
    assert_eq!((region.role.as_str(), region.items), ("outline", Some(85)));
    assert!(region.partial);
}

#[test]
fn a_scrollbar_without_evidence_does_not_claim_partial() {
    let mut children = rows("listitem", 85);
    children.push(scrollbar(true));
    let listbox = parent(with_ref(node("listbox", "Fonts"), "@s1:fonts"), children);
    let region = only_region(&window(vec![listbox]));
    assert_eq!((region.role.as_str(), region.items), ("listbox", Some(85)));
    assert!(!region.partial);
}

#[test]
fn a_scroll_action_without_evidence_does_not_claim_partial() {
    let mut list = parent(
        with_ref(node("list", "Files"), "@s1:files"),
        rows("listitem", 85),
    );
    list.presentation.available_actions = vec!["Scroll".into()];
    assert!(!only_region(&window(vec![list])).partial);
}

#[test]
fn a_linux_listbox_in_a_scroll_pane_is_the_one_region() {
    let listbox = parent(node("listbox", "Apps"), rows("listitem", 85));
    let mut pane = parent(node("scrollarea", ""), vec![listbox, scrollbar(true)]);
    pane.presentation.scrolls_vertically = Some(true);
    let region = only_region(&window(vec![pane]));
    assert_eq!((region.role.as_str(), region.items), ("listbox", Some(85)));
    assert!(region.partial);
}

#[test]
fn a_web_table_keeps_the_scroll_bar_of_the_area_around_its_document() {
    let table = parent(node("table", "Orders"), rows("row", 85));
    let document = parent(node("webarea", "Shop"), vec![table]);
    let mut area = parent(node("scrollarea", ""), vec![document, scrollbar(true)]);
    area.presentation.scrolls_vertically = Some(true);
    let region = only_region(&window(vec![area]));
    assert_eq!(region.role, "table");
    assert!(region.partial);
}

#[test]
fn paragraphs_beside_a_few_controls_are_not_a_list() {
    let mut page: Vec<AccessibilityNode> = (0..80)
        .map(|i| node("statictext", &format!("p{i}")))
        .collect();
    for (i, role) in ["button", "textfield", "checkbox", "combobox", "slider"]
        .iter()
        .enumerate()
    {
        page.push(with_ref(node(role, ""), &format!("@s1:c{i}")));
    }
    let area = parent(
        node("scrollarea", "Article"),
        vec![parent(node("group", ""), page)],
    );
    assert!(
        summarize(&window(vec![area]))
            .expect("contents")
            .regions
            .is_empty()
    );
}

#[test]
fn a_tree_with_no_ref_roles_omits_the_role_counts() {
    let text: Vec<AccessibilityNode> = (0..85)
        .map(|i| node("statictext", &format!("t{i}")))
        .collect();
    assert_eq!(
        summarize(&parent(node("window", "Help"), text.clone())),
        None
    );
    let list = parent(node("list", "Notes"), text);
    let contents = summarize(&parent(node("window", "Help"), vec![list])).expect("contents");
    let json = serde_json::to_value(&contents).unwrap();
    assert!(json.get("roles").is_none(), "{json}");
    assert!(
        contents
            .lines()
            .iter()
            .all(|line| !line.starts_with("roles:"))
    );
}
