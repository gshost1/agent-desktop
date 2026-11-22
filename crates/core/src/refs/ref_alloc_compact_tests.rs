use crate::refs::RefMap;
use crate::refs::ref_alloc::{RefAllocConfig, allocate_refs, transform_tree};
use crate::refs::ref_alloc_options::RefAllocOptions;
use crate::{AccessibilityNode, Rect};

fn bounded(role: &str, name: Option<&str>) -> AccessibilityNode {
    AccessibilityNode {
        ref_id: None,
        role: role.into(),
        identity: crate::NodeIdentity {
            name: name.map(str::to_string),
            ..Default::default()
        },
        presentation: crate::NodePresentation {
            bounds: Some(Rect {
                x: 4.0,
                y: 8.0,
                width: 120.0,
                height: 24.0,
            }),
            ..Default::default()
        },
        children_count: None,
        subtree_truncated: false,
        children: vec![],
    }
}

fn wrapped(children: Vec<AccessibilityNode>) -> AccessibilityNode {
    let mut wrapper = bounded("group", None);
    wrapper.children = children;
    wrapper
}

fn config(options: RefAllocOptions) -> RefAllocConfig<'static> {
    RefAllocConfig {
        capture: None,
        options,
        source: crate::refs::ref_alloc_source::RefAllocSource {
            pid: crate::ProcessId::new(9),
            app: Some("Fixture"),
            window_id: Some("w-9"),
            window_title: Some("Fixture"),
            window_bounds_hash: None,
            process_instance: Some("test-instance"),
            surface: crate::adapter::SnapshotSurface::Window,
        },
        scope: crate::refs::ref_alloc_scope::RefAllocScope {
            root_ref_id: None,
            path_prefix: &[],
        },
    }
}

fn allocate(tree: AccessibilityNode, options: RefAllocOptions) -> (AccessibilityNode, RefMap) {
    let mut refmap = RefMap::new();
    let out = allocate_refs(tree, &mut refmap, &config(options)).unwrap();
    (out, refmap)
}

fn compact() -> RefAllocOptions {
    RefAllocOptions {
        compact: true,
        ..Default::default()
    }
}

fn window_with(children: Vec<AccessibilityNode>) -> AccessibilityNode {
    let mut root = bounded("window", Some("Fixture"));
    root.children = children;
    root
}

fn refs_in_order(node: &AccessibilityNode, out: &mut Vec<String>) {
    if let Some(ref_id) = &node.ref_id {
        out.push(ref_id.clone());
    }
    for child in &node.children {
        refs_in_order(child, out);
    }
}

#[test]
fn compact_collapses_a_bounded_wrapper_once_the_snapshot_hides_bounds() {
    let tree = window_with(vec![wrapped(vec![wrapped(vec![bounded(
        "button",
        Some("Send"),
    )])])]);

    let (out, _) = allocate(tree, compact());

    assert_eq!(out.children.len(), 1);
    assert_eq!(out.children[0].role, "button");
    assert_eq!(out.children[0].identity.name.as_deref(), Some("Send"));
}

#[test]
fn compact_keeps_a_wrapper_whose_bounds_the_snapshot_shows() {
    let tree = window_with(vec![wrapped(vec![bounded("button", Some("Send"))])]);
    let options = RefAllocOptions {
        include_bounds: true,
        ..compact()
    };

    let (out, _) = allocate(tree, options);

    assert_eq!(out.children[0].role, "group");
    assert!(out.children[0].presentation.bounds.is_some());
}

#[test]
fn compact_collapses_a_wrapper_left_with_one_child_by_interactive_pruning() {
    let tree = window_with(vec![wrapped(vec![
        bounded("statictext", Some("Label")),
        bounded("button", Some("Send")),
    ])]);
    let options = RefAllocOptions {
        interactive_only: true,
        ..compact()
    };

    let (out, _) = allocate(tree, options);

    assert_eq!(out.children.len(), 1);
    assert_eq!(out.children[0].role, "button");
}

#[test]
fn compact_keeps_every_ref_id_and_entry_the_full_tree_allocates() {
    let tree = || {
        window_with(vec![
            wrapped(vec![wrapped(vec![bounded("button", Some("One"))])]),
            bounded("button", Some("Two")),
            wrapped(vec![bounded("checkbox", Some("Three"))]),
        ])
    };

    let (full, full_refs) = allocate(tree(), RefAllocOptions::default());
    let (shaped, shaped_refs) = allocate(tree(), compact());

    let mut full_order = Vec::new();
    refs_in_order(&full, &mut full_order);
    let mut shaped_order = Vec::new();
    refs_in_order(&shaped, &mut shaped_order);
    assert_eq!(shaped_order, full_order);
    assert_eq!(shaped.children.len(), 3);
    assert_eq!(shaped.children[0].role, "button");
    for ref_id in &full_order {
        let full_entry = full_refs.get(ref_id).unwrap();
        let shaped_entry = shaped_refs.get(ref_id).unwrap();
        assert_eq!(shaped_entry.scope.path, full_entry.scope.path);
        assert_eq!(shaped_entry.identity.name, full_entry.identity.name);
        assert_eq!(shaped_entry.identity.role, full_entry.identity.role);
    }
    let one = shaped.children[0].ref_id.as_deref().unwrap();
    assert_eq!(
        shaped_refs.get(one).unwrap().scope.path.as_slice(),
        [0, 0, 0]
    );
}

#[test]
fn raw_tree_compact_collapses_a_bounded_wrapper_once_bounds_are_stripped() {
    let tree = window_with(vec![wrapped(vec![bounded("button", Some("Go"))])]);

    let out = transform_tree(tree, false, false, true);

    assert_eq!(out.children.len(), 1);
    assert_eq!(out.children[0].role, "button");
}

#[test]
fn raw_tree_compact_keeps_a_wrapper_whose_bounds_are_kept() {
    let tree = window_with(vec![wrapped(vec![bounded("button", Some("Go"))])]);

    let out = transform_tree(tree, true, false, true);

    assert_eq!(out.children[0].role, "group");
}

#[test]
fn compact_keeps_a_lists_own_scroll_evidence_over_its_wrapper() {
    for outer in [Some(true), Some(false)] {
        for own in [Some(false), Some(true), None] {
            let mut list = bounded("list", None);
            list.presentation.scrolls_vertically = own;
            let mut wrapper = wrapped(vec![list]);
            wrapper.presentation.scrolls_vertically = outer;
            let tree = window_with(vec![wrapper]);
            let raw = transform_tree(tree.clone(), false, false, true);
            let (allocated, _) = allocate(tree, compact());
            for out in [raw, allocated] {
                assert_eq!(out.children[0].role, "list");
                assert_eq!(
                    out.children[0].presentation.scrolls_vertically,
                    own.or(outer)
                );
            }
        }
    }
}

#[test]
fn compact_limits_inherited_scroll_evidence_to_three_collapsed_wrappers() {
    for depth in 1..=4 {
        let mut child = bounded("list", None);
        for _ in 0..depth {
            child = wrapped(vec![child]);
        }
        child.presentation.scrolls_vertically = Some(true);
        let tree = window_with(vec![child]);
        let raw = transform_tree(tree.clone(), false, false, true);
        let (allocated, _) = allocate(tree, compact());
        for out in [raw, allocated] {
            assert_eq!(out.children[0].role, "list");
            assert_eq!(
                out.children[0].presentation.scrolls_vertically,
                (depth <= 3).then_some(true),
                "depth={depth}"
            );
        }
    }
}
