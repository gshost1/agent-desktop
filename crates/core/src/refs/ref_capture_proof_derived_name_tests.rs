use crate::locator::live_locator::{ObservationRoot, ObservedTree};
use crate::refs::ref_alloc::allocate_refs;
use crate::refs::ref_alloc_config::RefAllocConfig;
use crate::refs::ref_alloc_options::RefAllocOptions;
use crate::refs::ref_alloc_scope::RefAllocScope;
use crate::refs::ref_alloc_source::RefAllocSource;
use crate::refs::ref_capture_proof::CaptureProof;
use crate::{AccessibilityNode, NameSource, NodeDescriptor, RefEntry, RefMap, WindowInfo};

fn row_named_by_its_field(text: &str) -> AccessibilityNode {
    let mut row = crate::adapter::minimal_accessibility_node("treeitem", "");
    row.identity.name = None;
    row.presentation.descriptors = NodeDescriptor {
        name_source: Some(NameSource::ChildFieldValue),
        derived_name: Some(text.into()),
        ..NodeDescriptor::default()
    };
    row
}

fn observed(rows: Vec<AccessibilityNode>) -> ObservedTree {
    let window = WindowInfo {
        id: "w-1".into(),
        title: "Fixture".into(),
        app: "Fixture".into(),
        pid: crate::ProcessId::new(1),
        process_instance: Some("test-instance".into()),
        bounds: None,
        state: crate::WindowState::default(),
    };
    let mut root = crate::adapter::minimal_accessibility_node("window", "Fixture");
    root.children = rows;
    crate::adapter::observed_tree(&ObservationRoot::Window(&window), root)
        .expect("the fixture observes")
}

fn allocate(observed: &ObservedTree) -> (AccessibilityNode, Vec<RefEntry>) {
    let proof = CaptureProof::new(observed);
    let config = RefAllocConfig {
        options: RefAllocOptions {
            include_bounds: false,
            interactive_only: false,
            compact: false,
            force_renderer_accessibility: false,
        },
        source: RefAllocSource {
            pid: crate::ProcessId::new(1),
            app: Some("Fixture"),
            window_id: None,
            window_title: Some("Fixture"),
            window_bounds_hash: None,
            process_instance: Some("test-instance"),
            surface: crate::SnapshotSurface::Window,
        },
        scope: RefAllocScope {
            root_ref_id: None,
            path_prefix: &[],
        },
        capture: Some(&proof),
    };
    let projected = observed
        .clone()
        .into_accessibility_tree()
        .expect("the fixture projects");
    let mut refmap = RefMap::new();
    let tree = allocate_refs(projected, &mut refmap, &config).expect("refs allocate");
    let entries = (1..=refmap.len())
        .filter_map(|index| refmap.get(&format!("@e{index}")).cloned())
        .filter(|entry| entry.identity.role == "treeitem")
        .collect();
    (tree, entries)
}

#[test]
fn a_derived_name_is_displayed_and_located_but_never_recorded_as_identity() {
    let observed = observed(vec![
        row_named_by_its_field("alpha.txt"),
        row_named_by_its_field("beta.txt"),
    ]);
    let (tree, entries) = allocate(&observed);

    let rows: Vec<_> = tree.children.iter().collect();
    assert_eq!(rows[0].identity.name.as_deref(), Some("alpha.txt"));
    assert_eq!(
        rows[0].presentation.descriptors.name_source,
        Some(NameSource::ChildFieldValue)
    );
    let json = serde_json::to_value(rows[0]).expect("the node serializes");
    assert_eq!(json["name"], "alpha.txt");
    assert_eq!(json["name_source"], "child_field_value");
    assert!(json.get("derived_name").is_none());

    let query = crate::locator::LocatorQuery {
        identity: crate::locator::IdentityPredicate {
            name: Some("alpha".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(crate::locator::accessibility_node_matches(rows[0], &query));
    assert!(!crate::locator::accessibility_node_matches(rows[1], &query));

    assert_eq!(entries.len(), 2, "both rows are ref-able");
    for entry in &entries {
        assert_eq!(
            entry.identity.name, None,
            "a derived name is never identity"
        );
        assert_eq!(
            entry.identity.capture,
            crate::CaptureVerdict::Ambiguous,
            "two rows that differ only by a control value are twins by identity"
        );
    }
}

#[test]
fn the_capture_proof_never_sees_a_derived_name() {
    let observed = observed(vec![
        row_named_by_its_field("alpha.txt"),
        row_named_by_its_field("beta.txt"),
    ]);

    for node in observed.nodes.iter().skip(1) {
        assert!(
            node.evidence.name.known().is_none(),
            "the derived name rides beside an absent accessible name"
        );
    }
    let (_, entries) = allocate(&observed);
    let proof = CaptureProof::new(&observed);
    for (index, entry) in entries.iter().enumerate() {
        let counted = crate::refs::ref_full_predicate::FullPredicate::of_entry(entry).mask();
        assert_eq!(
            counted & crate::refs::ref_full_predicate::NAME,
            0,
            "the predicate the proof counts carries no name for a derived-name row"
        );
        assert_eq!(
            proof.verdict(entry, index + 1),
            crate::CaptureVerdict::Ambiguous
        );
    }
}
