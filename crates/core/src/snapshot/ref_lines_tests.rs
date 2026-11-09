use serde_json::json;

use super::{quote_text, render_ref_lines, replace_tree_with_lines};
use crate::{AccessibilityNode, Rect, adapter::minimal_accessibility_node};

fn node(role: &str, name: &str) -> AccessibilityNode {
    minimal_accessibility_node(role, name)
}

fn with_ref(mut node: AccessibilityNode, ref_id: &str) -> AccessibilityNode {
    node.ref_id = Some(ref_id.into());
    node
}

#[test]
fn ref_leads_the_line_and_depth_indents_two_spaces() {
    let mut row = node("group", "Row");
    row.children = vec![with_ref(node("button", "Open"), "@s1:e2")];
    let mut root = with_ref(node("window", "Main"), "@s1:e1");
    root.children = vec![row];

    assert_eq!(
        render_ref_lines(&root),
        vec![
            "@s1:e1 window \"Main\"",
            "  group \"Row\"",
            "    @s1:e2 button \"Open\"",
        ]
    );
}

#[test]
fn names_with_quotes_backslashes_and_breaks_stay_on_one_line() {
    assert_eq!(
        quote_text("Say \"hi\"\\now\nnext\tcol"),
        "\"Say \\\"hi\\\"\\\\now\\nnext\\tcol\""
    );
    assert_eq!(quote_text("bell\u{7}"), "\"bell\\u0007\"");
    assert_eq!(
        quote_text("one\u{2028}two\u{2029}"),
        "\"one\\u2028two\\u2029\""
    );
}

#[test]
fn bidirectional_controls_are_dropped_and_other_text_kept() {
    let mut display = node("statictext", "\u{200e}7\u{200f}");
    display.identity.value = Some("\u{202a}12\u{202c} × 3".into());

    assert_eq!(
        render_ref_lines(&display),
        vec!["statictext \"7\" value=\"12 × 3\""]
    );
}

#[test]
fn an_empty_name_prints_empty_quotes_and_an_absent_name_prints_nothing() {
    let empty = node("button", "");
    let mut absent = node("button", "");
    absent.identity.name = None;

    assert_eq!(render_ref_lines(&empty), vec!["button \"\""]);
    assert_eq!(render_ref_lines(&absent), vec!["button"]);
}

#[test]
fn fields_states_and_bounds_follow_the_documented_order() {
    let mut field = with_ref(node("textfield", "Search"), "@s1:e3");
    field.identity.value = Some("abc".into());
    field.identity.description = Some("Filters fonts".into());
    field.presentation.descriptors.placeholder = Some("Search".into());
    field.presentation.hint = Some("column 1 of 2".into());
    field.presentation.bounds = Some(Rect {
        x: 10.0,
        y: 20.5,
        width: 80.0,
        height: 24.0,
    });
    field.presentation.states = vec!["focused".into(), "editable".into()];
    field.presentation.available_actions = vec!["press".into()];

    assert_eq!(
        render_ref_lines(&field),
        vec![
            "@s1:e3 textfield \"Search\" value=\"abc\" desc=\"Filters fonts\" \
             placeholder=\"Search\" hint=\"column 1 of 2\" bounds=10,20.5,80x24 \
             [focused, editable]"
        ]
    );
}

#[test]
fn cut_branches_carry_their_markers() {
    let mut skeleton = node("group", "Sidebar");
    skeleton.children_count = Some(26);
    let mut budget = node("list", "Items");
    budget.subtree_truncated = true;

    assert_eq!(
        render_ref_lines(&skeleton),
        vec!["group \"Sidebar\" children_count=26"]
    );
    assert_eq!(
        render_ref_lines(&budget),
        vec!["list \"Items\" subtree_truncated"]
    );
}

#[test]
fn skeleton_fixture_renders_to_its_golden_lines() {
    let tree: AccessibilityNode = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/skeleton-tree.json"
    ))
    .unwrap();
    let golden = include_str!("../../../../tests/fixtures/skeleton-tree.lines.txt");

    assert_eq!(render_ref_lines(&tree).join("\n") + "\n", golden);
}

#[test]
fn replacing_the_tree_keeps_every_other_key() {
    let tree = serde_json::to_value(with_ref(node("button", "Send"), "@s9:e1")).unwrap();
    let mut data = json!({
        "app": "Font Book",
        "snapshot_id": "s9",
        "ref_count": 1,
        "complete": false,
        "truncated": true,
        "refmap_capped": true,
        "after_action": { "action": "click" },
        "tree": tree,
    });
    let mut expected = data.clone();
    let expected_body = expected.as_object_mut().unwrap();
    expected_body.remove("tree");
    expected_body.insert("lines".into(), json!(["@s9:e1 button \"Send\""]));

    assert!(replace_tree_with_lines(&mut data).unwrap());
    assert_eq!(data, expected);
}

#[test]
fn data_without_a_tree_is_left_alone() {
    let mut data = json!({ "action": "click" });
    let before = data.clone();

    assert!(!replace_tree_with_lines(&mut data).unwrap());
    assert!(!replace_tree_with_lines(&mut json!("text")).unwrap());
    assert_eq!(data, before);
}

#[test]
fn a_tree_that_does_not_read_back_leaves_data_unchanged() {
    let mut data = json!({ "snapshot_id": "s9", "tree": { "name": "no role" } });
    let before = data.clone();

    assert!(replace_tree_with_lines(&mut data).is_err());
    assert_eq!(data, before);
}
