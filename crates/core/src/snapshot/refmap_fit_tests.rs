use super::*;
use crate::refs::RefMap;
use crate::refs::refs_store::RefStore;
use crate::refs::refs_test_support::HomeGuard;
use crate::snapshot::tokenless_capture_support::{
    CaptureAdapter, drill_opts, group, node, resolve, window,
};

const NAME_PAD: usize = 400;
const BUTTONS_PER_GROUP: usize = 1_000;

fn padded(index: usize) -> String {
    format!("Item {index:05} {}", "x".repeat(NAME_PAD))
}

fn buttons(range: std::ops::Range<usize>) -> Vec<AccessibilityNode> {
    range.map(|index| node("button", &padded(index))).collect()
}

/// Two groups of buttons whose refs together serialize well past 1 MB: the
/// first group fits under the cap on its own, the second does not.
fn oversized_window() -> AccessibilityNode {
    window(vec![
        group(buttons(0..BUTTONS_PER_GROUP)),
        group(buttons(BUTTONS_PER_GROUP..2 * BUTTONS_PER_GROUP)),
    ])
}

fn ref_ids_in_order(node: &AccessibilityNode, out: &mut Vec<String>) {
    if let Some(ref_id) = &node.ref_id {
        out.push(ref_id.clone());
    }
    for child in &node.children {
        ref_ids_in_order(child, out);
    }
}

fn local_ref(qualified: &str) -> String {
    let (_, local) = crate::refs::ref_token::resolve_ref_target(qualified).unwrap();
    local
}

fn allocate(tree: AccessibilityNode) -> (AccessibilityNode, RefMap) {
    let config = crate::refs::ref_alloc::RefAllocConfig {
        options: Default::default(),
        source: crate::refs::ref_alloc_source::RefAllocSource {
            pid: crate::ProcessId::new(1),
            app: Some("Fixture"),
            window_id: None,
            window_title: Some("Fixture"),
            window_bounds_hash: None,
            process_instance: Some("test-instance"),
            surface: crate::adapter::SnapshotSurface::Window,
        },
        scope: crate::refs::ref_alloc_scope::RefAllocScope {
            root_ref_id: None,
            path_prefix: &[],
        },
        capture: None,
    };
    let mut refmap = RefMap::new();
    let tree = crate::refs::ref_alloc::allocate_refs(tree, &mut refmap, &config).unwrap();
    (tree, refmap)
}

fn small_tree() -> (AccessibilityNode, RefMap) {
    let tree = window(vec![
        group(vec![node("button", "A"), node("button", "B")]),
        group(vec![node("button", "C"), node("button", "D")]),
        node("button", "E"),
    ]);
    allocate(tree)
}

fn cap_for_refs(refmap: &RefMap, count: usize) -> usize {
    let entries: usize = (1..=count)
        .map(|index| {
            refmap
                .serialized_entry_len(&format!("@e{index}"))
                .unwrap()
                .unwrap()
        })
        .sum();
    refmap.serialized_len_without_entries().unwrap() + entries + count - 1
}

#[test]
fn an_oversized_snapshot_succeeds_as_a_partial_tree_with_resolvable_refs() {
    let _guard = HomeGuard::new();
    let adapter = CaptureAdapter::new(oversized_window());

    let result = crate::snapshot::run(&adapter, &drill_opts(), Some("Fixture"), None)
        .expect("an oversized snapshot is partial, not an error");

    assert!(!result.complete);
    assert!(result.notes.refmap_capped);
    assert!(result.refmap.serialized_len().unwrap() <= MAX_REFMAP_BYTES as usize);

    let mut presented = Vec::new();
    ref_ids_in_order(&result.tree, &mut presented);
    let local: Vec<String> = presented.iter().map(|id| local_ref(id)).collect();
    let expected: Vec<String> = (1..=local.len())
        .map(|index| format!("@e{index}"))
        .collect();
    assert_eq!(local, expected, "kept refs are the allocation-order prefix");
    assert_eq!(result.refmap.len(), local.len());
    assert!(local.len() > BUTTONS_PER_GROUP && local.len() < 2 * BUTTONS_PER_GROUP);
    assert!(
        result
            .refmap
            .get(&format!("@e{}", local.len() + 1))
            .is_none()
    );

    assert!(result.tree.subtree_truncated);
    let first = &result.tree.children[0];
    let second = &result.tree.children[1];
    assert!(!first.subtree_truncated);
    assert_eq!(first.children.len(), BUTTONS_PER_GROUP);
    assert!(second.subtree_truncated);
    assert_eq!(first.children.len() + second.children.len(), local.len());

    let snapshot_id = result.snapshot_id.clone().unwrap();
    let stored = RefStore::new()
        .unwrap()
        .load_snapshot(&snapshot_id)
        .unwrap();
    assert_eq!(stored.len(), local.len());
    for ref_id in [local.first().unwrap(), local.last().unwrap()] {
        let entry = stored.get(ref_id).expect("a presented ref is stored");
        assert!(entry.identity.unique_at_capture);
        resolve(&adapter, entry).expect("a stored ref resolves");
    }
}

#[test]
fn the_same_tree_always_cuts_at_the_same_ref() {
    let _guard = HomeGuard::new();
    let adapter = CaptureAdapter::new(oversized_window());
    let first = crate::snapshot::run(&adapter, &drill_opts(), Some("Fixture"), None).unwrap();
    let second = crate::snapshot::run(&adapter, &drill_opts(), Some("Fixture"), None).unwrap();
    assert_eq!(first.refmap.len(), second.refmap.len());
    assert_eq!(
        first.tree.children[1].children.len(),
        second.tree.children[1].children.len()
    );
}

#[test]
fn a_refmap_under_the_cap_is_left_untouched() {
    let (mut tree, mut refmap) = small_tree();
    let before = serde_json::to_value(&tree).unwrap();
    let cap = refmap.serialized_len().unwrap();

    assert!(!fit_refmap_within(&mut tree, &mut refmap, cap).unwrap());
    assert_eq!(serde_json::to_value(&tree).unwrap(), before);
    assert_eq!(refmap.len(), 5);
}

#[test]
fn a_cut_marks_every_ancestor_and_drops_every_later_node() {
    let (mut tree, mut refmap) = small_tree();
    let cap = cap_for_refs(&refmap, 3);

    assert!(fit_refmap_within(&mut tree, &mut refmap, cap).unwrap());
    assert_eq!(refmap.serialized_len().unwrap(), cap);

    let mut kept = Vec::new();
    ref_ids_in_order(&tree, &mut kept);
    assert_eq!(kept, ["@e1", "@e2", "@e3"]);
    assert_eq!(refmap.len(), 3);
    assert!(tree.subtree_truncated);
    assert_eq!(
        tree.children.len(),
        2,
        "the button after the cut is dropped"
    );
    assert!(!tree.children[0].subtree_truncated);
    assert!(tree.children[1].subtree_truncated);
    assert_eq!(tree.children[1].children.len(), 1);
}

#[test]
fn a_cut_before_a_groups_first_child_keeps_the_group_marked_and_empty() {
    let (mut tree, mut refmap) = small_tree();
    let cap = cap_for_refs(&refmap, 2);

    assert!(fit_refmap_within(&mut tree, &mut refmap, cap).unwrap());
    assert_eq!(tree.children.len(), 2);
    assert!(tree.children[1].subtree_truncated);
    assert!(tree.children[1].children.is_empty());
}

#[test]
fn a_root_ref_that_cannot_fit_leaves_the_bare_root_marked() {
    let (tree, _) = small_tree();
    let mut root = node("button", "Root");
    root.children = vec![tree];
    let (mut root, mut refmap) = allocate(root);

    assert!(fit_refmap_within(&mut root, &mut refmap, 1).unwrap());
    assert!(root.ref_id.is_none());
    assert!(root.children.is_empty());
    assert!(root.subtree_truncated);
    assert!(refmap.is_empty());
}
