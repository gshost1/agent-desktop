use super::*;
use crate::adapter::minimal_accessibility_node as node;
use crate::{SnapshotNotes, SnapshotOwner, SnapshotSurface, WindowInfo, refs::RefMap};

fn window(id: &str) -> SnapshotOwner {
    SnapshotOwner::Window(WindowInfo {
        id: id.into(),
        title: "Doc".into(),
        app: "App".into(),
        pid: crate::ProcessId::new(7),
        process_instance: Some("start-1".into()),
        bounds: None,
        state: crate::WindowState::default(),
    })
}

fn presentation() -> DiffPresentation {
    DiffPresentation {
        surface: SnapshotSurface::Window,
        max_depth: 10,
        interactive_only: false,
        compact: false,
        skeleton: false,
    }
}

fn result(children: Vec<AccessibilityNode>, complete: bool) -> SnapshotResult {
    let mut root = node("window", "Doc");
    root.children = children;
    SnapshotResult {
        tree: root,
        refmap: RefMap::new(),
        owner: window("w-1"),
        snapshot_id: Some("s2".into()),
        complete,
        nodes_observed: 0,
        notes: SnapshotNotes::default(),
    }
}

fn baseline(children: Vec<AccessibilityNode>, complete: bool) -> DiffBaseline {
    let before = result(children, complete);
    DiffBaseline {
        format: 1,
        owner: DiffOwner::of(&before.owner),
        presentation: presentation(),
        complete,
        rows: diff_rows::baseline_rows(&before.tree),
    }
}

fn with_ref(mut node: AccessibilityNode, ref_id: &str) -> AccessibilityNode {
    node.ref_id = Some(ref_id.into());
    node
}

fn field(value: &str) -> AccessibilityNode {
    let mut field = node("textfield", "Name");
    field.identity.value = Some(value.into());
    field
}

fn names(rows: &[AccessibilityNode]) -> Vec<&str> {
    rows.iter()
        .filter_map(|row| row.identity.name.as_deref())
        .collect()
}

#[test]
fn identical_trees_are_unchanged_and_still_report_focus() {
    let mut focused = with_ref(field("x"), "@s1:e1");
    focused.presentation.states = vec!["focused".into()];
    let before = baseline(vec![focused.clone()], true);
    let mut now = focused;
    now.ref_id = Some("@s2:e1".into());
    let diff = compare("s1", &before, &result(vec![now], true), presentation());
    assert_eq!(diff.status, DiffStatus::Unchanged);
    assert!(diff.added.is_empty() && diff.changed.is_empty() && diff.removed.is_empty());
    assert_eq!(diff.focused.unwrap().ref_id.as_deref(), Some("@s2:e1"));
}

#[test]
fn a_new_node_is_added_with_its_fresh_ref() {
    let before = baseline(vec![with_ref(node("button", "Save"), "@e1")], true);
    let now = result(
        vec![
            with_ref(node("button", "Save"), "@s2:e1"),
            with_ref(node("button", "Close"), "@s2:e2"),
        ],
        true,
    );
    let diff = compare("s1", &before, &now, presentation());
    assert_eq!(diff.status, DiffStatus::Changed);
    assert_eq!(names(&diff.added), ["Close"]);
    assert_eq!(diff.added[0].ref_id.as_deref(), Some("@s2:e2"));
    assert!(diff.changed.is_empty() && diff.removed.is_empty());
}

#[test]
fn a_gone_node_is_removed_without_a_ref() {
    let before = baseline(
        vec![
            with_ref(node("button", "Save"), "@e1"),
            node("statictext", "Loading"),
        ],
        true,
    );
    let now = result(vec![with_ref(node("button", "Save"), "@s2:e1")], true);
    let diff = compare("s1", &before, &now, presentation());
    assert_eq!(names(&diff.removed), ["Loading"]);
    assert!(diff.removed[0].ref_id.is_none());
    assert!(diff.added.is_empty());
}

#[test]
fn a_matched_node_whose_value_changed_is_changed_with_the_earlier_value() {
    let before = baseline(vec![with_ref(field("ab"), "@e1")], true);
    let now = result(vec![with_ref(field("abc"), "@s2:e1")], true);
    let diff = compare("s1", &before, &now, presentation());
    assert_eq!(diff.status, DiffStatus::Changed);
    assert_eq!(diff.changed.len(), 1);
    let row = &diff.changed[0];
    assert_eq!(row.fields, ["value"]);
    assert_eq!(row.now.ref_id.as_deref(), Some("@s2:e1"));
    assert_eq!(row.before.identity.value.as_deref(), Some("ab"));
    assert!(diff.added.is_empty() && diff.removed.is_empty());
}

#[test]
fn a_node_whose_identity_changed_is_removed_and_added_never_changed() {
    let before = baseline(vec![with_ref(node("button", "Save"), "@e1")], true);
    let now = result(vec![with_ref(node("button", "Saved"), "@s2:e1")], true);
    let diff = compare("s1", &before, &now, presentation());
    assert!(diff.changed.is_empty());
    assert_eq!(names(&diff.removed), ["Save"]);
    assert_eq!(names(&diff.added), ["Saved"]);
}

#[test]
fn twins_are_never_paired_by_position() {
    let before = baseline(vec![field("a"), field("b")], true);
    let now = result(vec![field("a"), field("c")], true);
    let diff = compare("s1", &before, &now, presentation());
    assert!(diff.changed.is_empty());
    let value = |rows: &[AccessibilityNode]| -> Vec<String> {
        rows.iter()
            .filter_map(|row| row.identity.value.clone())
            .collect()
    };
    assert_eq!(value(&diff.removed), ["b"]);
    assert_eq!(value(&diff.added), ["c"]);
}

#[test]
fn a_twin_that_appears_next_to_an_unchanged_node_is_only_added() {
    let before = baseline(vec![field("a")], true);
    let now = result(vec![field("a"), field("a")], true);
    let diff = compare("s1", &before, &now, presentation());
    assert_eq!(diff.added.len(), 1);
    assert!(diff.changed.is_empty() && diff.removed.is_empty());
}

#[test]
fn an_incomplete_side_is_never_unchanged() {
    let rows = || vec![node("button", "Save")];
    for (before_complete, now_complete) in [(false, true), (true, false), (false, false)] {
        let diff = compare(
            "s1",
            &baseline(rows(), before_complete),
            &result(rows(), now_complete),
            presentation(),
        );
        assert_eq!(
            diff.status,
            DiffStatus::Incomplete {
                baseline: !before_complete,
                snapshot: !now_complete
            }
        );
    }
}

#[test]
fn another_window_or_another_shape_is_not_comparable() {
    let before = baseline(vec![node("button", "Save")], true);
    let mut moved = result(vec![node("button", "Save")], true);
    moved.owner = window("w-2");
    let diff = compare("s1", &before, &moved, presentation());
    assert_eq!(diff.status, DiffStatus::NotComparable("owner_changed"));

    let reshaped = DiffPresentation {
        interactive_only: true,
        ..presentation()
    };
    let diff = compare("s1", &before, &result(vec![], true), reshaped);
    assert_eq!(diff.status, DiffStatus::NotComparable("presentation_changed"));
    assert!(diff.added.is_empty() && diff.removed.is_empty());
}
