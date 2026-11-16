use super::super::{
    LocatorMaterialization, LocatorResolveRequest, LocatorSelection, evaluate_locator_tree,
    test_support::{evidence, node, tree},
};
use crate::commands::query::parse_selector;

fn request() -> LocatorResolveRequest {
    LocatorResolveRequest {
        selection: LocatorSelection::First,
        deadline: crate::Deadline::from_duration(std::time::Duration::from_secs(5)).unwrap(),
        max_raw_depth: 50,
        surface: None,
        materialization: LocatorMaterialization::None,
    }
}

/// A window at depth 0 holding `Saved!` at depth 2 beside an unrelated
/// branch four levels deep, and an unnamed button whose label is its child.
fn window_tree() -> super::super::ObservedTree {
    tree(
        vec![
            node(0, evidence("window", Some("Doc")), vec![1, 3, 7], &[]),
            node(1, evidence("group", None), vec![2], &[0]),
            node(2, evidence("text", Some("Saved!")), vec![], &[0, 0]),
            node(3, evidence("group", None), vec![4], &[1]),
            node(4, evidence("group", None), vec![5], &[1, 0]),
            node(5, evidence("group", None), vec![6], &[1, 0, 0]),
            node(6, evidence("text", Some("deep")), vec![], &[1, 0, 0, 0]),
            node(7, evidence("button", None), vec![8], &[2]),
            node(8, evidence("text", Some("Done")), vec![], &[2, 0]),
        ],
        vec![0],
        true,
    )
}

fn first_match_depth(selector: &str) -> u8 {
    let resolution =
        evaluate_locator_tree(window_tree(), &parse_selector(selector), &request()).unwrap();
    resolution.matches[0].logical_depth
}

#[test]
fn a_text_match_reaches_its_shallowest_text_not_the_whole_subtree() {
    assert_eq!(first_match_depth(":Saved!"), 2);
}

#[test]
fn a_match_labelled_by_a_child_reaches_the_label() {
    assert_eq!(first_match_depth("button:Done"), 2);
}

#[test]
fn a_role_match_reaches_its_own_depth() {
    assert_eq!(first_match_depth("button"), 1);
}
