use super::ObservedSubtree;
use crate::locator::live_locator::test_support::{evidence, window};
use crate::locator::live_locator::{
    LocatorMaterialization, LocatorResolveRequest, LocatorSelection, LocatorStats,
    ObservationSource, ObservedTree, evaluate_locator_tree,
};
use crate::{IdentityPredicate, locator::LocatorQuery};

fn leaf(role: &str, name: &str) -> ObservedSubtree {
    ObservedSubtree::new(evidence(role, Some(name)), Vec::new(), true, None)
}

fn panel_with_deferred_list(rows_complete: bool) -> ObservedTree {
    let list = ObservedSubtree::new(evidence("list", None), Vec::new(), true, None)
        .with_source_child_index(1);
    let mut root = ObservedSubtree::new(
        evidence("sheet", Some("Panel")),
        vec![
            leaf("button", "Before").with_source_child_index(0),
            list,
            leaf("combobox", "Where:").with_source_child_index(2),
        ],
        true,
        None,
    );
    let rows = vec![leaf("row", "f0000.txt").with_source_child_index(0)];
    assert!(root.attach_deferred_children(&[1], rows, rows_complete, Some(3000)));
    assert!(!root.attach_deferred_children(&[7], Vec::new(), true, None));
    let source = ObservationSource::Window {
        window: window(),
        surface: crate::SnapshotSurface::Sheet,
        renderer_accessibility_forced: false,
    };
    ObservedTree::from_roots(vec![root], source, LocatorStats::default(), rows_complete).unwrap()
}

fn query(name: &str) -> LocatorQuery {
    LocatorQuery {
        identity: IdentityPredicate {
            name: Some(name.into()),
            ..Default::default()
        },
        ..LocatorQuery::default()
    }
}

fn request(selection: LocatorSelection) -> LocatorResolveRequest {
    LocatorResolveRequest {
        selection,
        deadline: crate::Deadline::after(5_000).unwrap(),
        max_raw_depth: 50,
        surface: None,
        materialization: LocatorMaterialization::None,
    }
}

#[test]
fn deferred_rows_keep_their_place_in_document_order() {
    let tree = panel_with_deferred_list(true);
    let roles: Vec<_> = tree
        .nodes
        .iter()
        .map(|node| node.evidence.role.known().cloned().unwrap_or_default())
        .collect();
    assert_eq!(roles, ["sheet", "button", "list", "row", "combobox"]);
    assert_eq!(tree.nodes[3].path.as_slice(), &[1, 0]);
    assert!(tree.is_complete());
}

#[test]
fn unread_deferred_rows_leave_the_answer_after_them_unproven() {
    let tree = panel_with_deferred_list(false);
    assert!(!tree.nodes[0].completeness.subtree_complete);
    assert!(tree.nodes[4].completeness.subtree_complete);

    let after = evaluate_locator_tree(
        tree.clone(),
        &query("Where:"),
        &request(LocatorSelection::First),
    )
    .unwrap();
    assert_eq!(after.meta.total_matches, 1);
    assert!(
        !after.meta.selection_complete,
        "a match after unread rows is not proven first or unique"
    );

    let before = evaluate_locator_tree(
        tree.clone(),
        &query("Before"),
        &request(LocatorSelection::First),
    )
    .unwrap();
    assert!(before.meta.selection_complete);

    let all = evaluate_locator_tree(
        tree,
        &query("Before"),
        &request(LocatorSelection::All { limit: Some(50) }),
    )
    .unwrap();
    assert!(!all.meta.selection_complete);
}
