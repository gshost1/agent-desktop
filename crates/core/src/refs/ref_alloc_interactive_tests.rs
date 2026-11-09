use super::*;
use crate::NameSource;

fn node(role: &str, name: Option<&str>, actions: &[&str]) -> AccessibilityNode {
    let mut node = AccessibilityNode {
        ref_id: None,
        role: role.into(),
        identity: crate::NodeIdentity {
            name: name.map(str::to_owned),
            ..Default::default()
        },
        presentation: Default::default(),
        children_count: None,
        subtree_truncated: false,
        children: vec![],
    };
    node.presentation.available_actions = actions.iter().map(|a| a.to_string()).collect();
    node
}

fn contained(mut node: AccessibilityNode) -> AccessibilityNode {
    node.presentation.descriptors.name_source = Some(NameSource::ContainedText);
    node
}

fn allocate(root: AccessibilityNode, interactive_only: bool) -> AccessibilityNode {
    let mut refmap = RefMap::new();
    let config = RefAllocConfig {
        capture: None,
        options: crate::refs::ref_alloc_options::RefAllocOptions {
            interactive_only,
            ..Default::default()
        },
        source: crate::refs::ref_alloc_source::RefAllocSource {
            pid: crate::ProcessId::new(7),
            app: None,
            window_id: None,
            window_title: None,
            window_bounds_hash: None,
            process_instance: Some("test-instance"),
            surface: crate::adapter::SnapshotSurface::Web,
        },
        scope: crate::refs::ref_alloc_scope::RefAllocScope {
            root_ref_id: None,
            path_prefix: &[],
        },
    };
    allocate_refs(root, &mut refmap, &config).unwrap()
}

fn page() -> AccessibilityNode {
    let mut paragraph = contained(node("paragraph", Some("Conceived by Ada, 1989."), &[]));
    paragraph.children = vec![node("link", Some("Ada"), &["Click"])];
    let row = contained(node("group", Some("Springfield, IL"), &["Click"]));
    let mut heading = node("heading", Some("History of Ada"), &[]);
    heading.children = vec![node("link", Some("Ada"), &["Click"])];
    let mut root = node("webarea", Some("Page"), &[]);
    root.children = vec![paragraph, row, heading];
    root
}

#[test]
fn interactive_only_drops_the_contained_text_of_a_container_without_a_ref() {
    let out = allocate(page(), true);

    let paragraph = &out.children[0];
    assert_eq!(paragraph.identity.name, None);
    assert_eq!(paragraph.presentation.descriptors.name_source, None);
    assert!(paragraph.children[0].ref_id.is_some());
    let row = &out.children[1];
    assert!(row.ref_id.is_some());
    assert_eq!(row.identity.name.as_deref(), Some("Springfield, IL"));
    assert_eq!(
        row.presentation.descriptors.name_source,
        Some(NameSource::ContainedText)
    );
    assert_eq!(
        out.children[2].identity.name.as_deref(),
        Some("History of Ada")
    );
}

#[test]
fn a_full_snapshot_keeps_the_contained_text_of_every_block() {
    let out = allocate(page(), false);

    assert_eq!(
        out.children[0].identity.name.as_deref(),
        Some("Conceived by Ada, 1989.")
    );
}

#[test]
fn interactive_only_keeps_a_name_derived_from_a_child_field() {
    let mut row = node("row", Some("report.txt"), &[]);
    row.presentation.descriptors.name_source = Some(NameSource::ChildFieldValue);
    row.children = vec![node("textfield", Some("report.txt"), &["Click"])];
    let mut root = node("webarea", Some("Page"), &[]);
    root.children = vec![row];

    let out = allocate(root, true);

    let row = &out.children[0];
    assert_eq!(row.identity.name.as_deref(), Some("report.txt"));
    assert_eq!(
        row.presentation.descriptors.name_source,
        Some(NameSource::ChildFieldValue)
    );
}
