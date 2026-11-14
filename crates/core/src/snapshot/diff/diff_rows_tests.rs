use super::*;
use crate::adapter::minimal_accessibility_node as node;

fn tree() -> AccessibilityNode {
    let mut window = node("window", "Doc");
    let mut group = node("group", "");
    group.identity.name = None;
    let mut field = node("textfield", "Name");
    field.ref_id = Some("@e1".into());
    field.presentation.states = vec!["focused".into()];
    field.presentation.bounds = Some(crate::Rect {
        x: 1.0,
        y: 2.0,
        width: 3.0,
        height: 4.0,
    });
    field.presentation.available_actions = vec!["SetValue".into()];
    let mut nested = node("group", "");
    nested.identity.name = None;
    nested.presentation.states = vec!["focused".into()];
    let mut inner = node("statictext", "deep");
    inner.presentation.states = vec!["focused".into()];
    nested.children = vec![inner];
    group.children = vec![field, nested];
    window.children = vec![group];
    window
}

#[test]
fn structure_only_nodes_are_not_rows_and_rows_have_no_children() {
    let rows = reported_rows(&tree());
    let roles: Vec<&str> = rows.iter().map(|row| row.role.as_str()).collect();
    assert_eq!(roles, ["window", "textfield", "group", "statictext"]);
    assert!(rows.iter().all(|row| row.children.is_empty()));
}

#[test]
fn baseline_rows_drop_bounds_and_actions_but_keep_refs() {
    let rows = baseline_rows(&tree());
    let field = &rows[1];
    assert_eq!(field.ref_id.as_deref(), Some("@e1"));
    assert!(field.presentation.bounds.is_none());
    assert!(field.presentation.available_actions.is_empty());
}

#[test]
fn the_focused_row_is_the_deepest_focused_node() {
    let focused = focused_row(&tree()).unwrap();
    assert_eq!(focused.identity.name.as_deref(), Some("deep"));
    assert!(focused_row(&node("window", "Doc")).is_none());
}
