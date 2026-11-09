use super::*;

fn partial_result() -> snapshot::SnapshotResult {
    let mut tree = crate::adapter::minimal_accessibility_node("window", "Fixture");
    tree.subtree_truncated = true;
    snapshot::SnapshotResult {
        tree,
        refmap: crate::RefMap::new(),
        owner: crate::SnapshotOwner::Window(crate::WindowInfo {
            id: "w-1".into(),
            title: "Fixture".into(),
            app: "Fixture".into(),
            pid: crate::ProcessId::new(1),
            process_instance: None,
            bounds: None,
            state: crate::WindowState::default(),
        }),
        snapshot_id: None,
        complete: false,
        nodes_observed: 42,
        notes: crate::SnapshotNotes::default(),
    }
}

#[test]
fn a_refmap_cut_is_reported_beside_the_partial_fields() {
    let mut result = partial_result();
    let budget_cut = format_snapshot_fields(&result, None, None).unwrap();
    assert!(budget_cut.get("refmap_capped").is_none());

    result.notes.refmap_capped = true;
    let capped = format_snapshot_fields(&result, None, None).unwrap();
    assert_eq!(capped["refmap_capped"], true);
    assert_eq!(capped["complete"], false);
    assert_eq!(capped["truncated"], true);
    assert_eq!(capped["nodes_observed"], 42);
    assert_eq!(capped["tree"]["subtree_truncated"], true);
}
