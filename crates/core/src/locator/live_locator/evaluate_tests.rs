use super::evaluate_locator_tree;
use crate::locator::{
    LocatorQuery,
    live_locator::{
        LocatorMaterialization, LocatorResolveRequest, LocatorSelection, ObservedTree,
        test_support::{evidence, node, tree},
    },
};

fn by_name(name: &str) -> LocatorQuery {
    LocatorQuery {
        identity: crate::IdentityPredicate {
            name: Some(crate::tree::search_text::normalize(name)),
            ..Default::default()
        },
        ..LocatorQuery::default()
    }
}

fn request(selection: LocatorSelection) -> LocatorResolveRequest {
    LocatorResolveRequest {
        selection,
        deadline: crate::Deadline::after(1_000).unwrap(),
        max_raw_depth: 50,
        surface: None,
        materialization: LocatorMaterialization::None,
    }
}

/// A window whose read stopped inside a list after the button `first`,
/// so the read is complete up to the button and no further.
fn cut_after(first: &str) -> ObservedTree {
    let mut root = node(0, evidence("window", None), vec![1, 2], &[]);
    root.completeness.subtree_complete = false;
    let mut list = node(2, evidence("list", None), Vec::new(), &[1]);
    list.completeness.subtree_complete = false;
    list.children_count = Some(100);
    tree(
        vec![
            root,
            node(1, evidence("button", Some(first)), Vec::new(), &[0]),
            list,
        ],
        vec![0],
        false,
    )
}

fn settled(tree: ObservedTree, selection: LocatorSelection) -> (String, bool) {
    let resolution = evaluate_locator_tree(tree, &by_name("Save"), &request(selection)).unwrap();
    let name = resolution
        .matches
        .first()
        .map(|found| found.data.name.clone())
        .unwrap_or_default();
    (name, resolution.meta.selection_complete)
}

#[test]
fn a_longer_name_first_in_a_cut_read_does_not_settle_a_whole_name_first() {
    assert_eq!(
        settled(
            cut_after("Save As"),
            LocatorSelection::FirstPreferringExactName
        ),
        ("Save As".to_string(), false)
    );
    assert_eq!(
        settled(cut_after("Save As"), LocatorSelection::First),
        ("Save As".to_string(), true)
    );
}

#[test]
fn a_whole_name_first_in_a_cut_read_settles_it() {
    assert_eq!(
        settled(
            cut_after("Save"),
            LocatorSelection::FirstPreferringExactName
        ),
        ("Save".to_string(), true)
    );
}

#[test]
fn a_longer_name_outside_unread_rows_settles_it_without_the_rows() {
    let mut rows_unread = cut_after("Save As");
    rows_unread.mark_rows_unread();
    assert_eq!(
        settled(rows_unread, LocatorSelection::FirstPreferringExactName),
        ("Save As".to_string(), true)
    );
}

#[test]
fn a_whole_name_after_a_longer_one_is_the_pick_in_a_complete_read() {
    let complete = tree(
        vec![
            node(0, evidence("window", None), vec![1, 2], &[]),
            node(1, evidence("button", Some("Save As")), Vec::new(), &[0]),
            node(2, evidence("button", Some("Save")), Vec::new(), &[1]),
        ],
        vec![0],
        true,
    );
    assert_eq!(
        settled(complete, LocatorSelection::FirstPreferringExactName),
        ("Save".to_string(), true)
    );
}
