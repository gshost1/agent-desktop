use super::super::{
    LocatorMaterialization, LocatorResolveRequest, LocatorSelection, evaluate_locator_tree,
    test_support::{evidence, node, tree},
};
use crate::commands::query::parse_selector;
use crate::locator::{ContainmentPredicate, IdentityPredicate, LocatorQuery};

fn request(materialization: LocatorMaterialization) -> LocatorResolveRequest {
    LocatorResolveRequest {
        selection: LocatorSelection::First,
        deadline: crate::Deadline::from_duration(std::time::Duration::from_secs(5)).unwrap(),
        max_raw_depth: 50,
        surface: None,
        materialization,
    }
}

/// A window at depth 0 holding `Saved!` at depth 2 beside an unrelated
/// branch four levels deep, an unnamed button whose label is its child, and
/// a button four levels down at the end of the deep branch.
fn window_tree() -> super::super::ObservedTree {
    tree(
        vec![
            node(0, evidence("window", Some("Doc")), vec![1, 3, 7], &[]),
            node(1, evidence("group", None), vec![2], &[0]),
            node(2, evidence("text", Some("Saved!")), vec![], &[0, 0]),
            node(3, evidence("group", None), vec![4], &[1]),
            node(4, evidence("group", None), vec![5], &[1, 0]),
            node(5, evidence("group", None), vec![6], &[1, 0, 0]),
            node(6, evidence("button", Some("deep")), vec![], &[1, 0, 0, 0]),
            node(7, evidence("button", None), vec![8], &[2]),
            node(8, evidence("text", Some("Done")), vec![], &[2, 0]),
        ],
        vec![0],
        true,
    )
}

fn depth_of(query: &LocatorQuery) -> Option<u8> {
    evaluate_locator_tree(
        window_tree(),
        query,
        &request(LocatorMaterialization::MatchDepth),
    )
    .unwrap()
    .match_depth
}

fn window_with(nested: LocatorQuery) -> LocatorQuery {
    LocatorQuery {
        identity: IdentityPredicate {
            role: Some("window".into()),
            ..IdentityPredicate::default()
        },
        containment: ContainmentPredicate {
            has: Some(Box::new(nested)),
            has_not: None,
        },
        ..LocatorQuery::default()
    }
}

#[test]
fn a_text_match_reaches_its_shallowest_text_not_the_whole_subtree() {
    assert_eq!(depth_of(&parse_selector(":Saved!")), Some(2));
}

#[test]
fn a_match_labelled_by_a_child_reaches_the_label() {
    assert_eq!(depth_of(&parse_selector("button:Done")), Some(2));
}

#[test]
fn the_shallowest_match_sets_the_depth_not_the_first_in_document_order() {
    assert_eq!(depth_of(&parse_selector("button")), Some(1));
}

#[test]
fn a_containment_match_reaches_the_node_that_satisfies_it() {
    let shallow = window_with(parse_selector("button"));
    let deep = window_with(parse_selector("button:deep"));

    assert_eq!(depth_of(&shallow), Some(1));
    assert_eq!(depth_of(&deep), Some(4));
}

#[test]
fn a_resolution_that_does_not_ask_computes_no_depth() {
    let resolution = evaluate_locator_tree(
        window_tree(),
        &parse_selector(":Saved!"),
        &request(LocatorMaterialization::None),
    )
    .unwrap();

    assert_eq!(resolution.meta.total_matches, 3);
    assert_eq!(resolution.match_depth, None);
}
