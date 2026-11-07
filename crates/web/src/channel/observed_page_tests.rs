use agent_desktop_core::{AppInfo, LocatorField, ObservationRoot, ProcessId, SnapshotSurface};

use super::{node_evidence, observed_page};
use crate::channel::test_support::node;

fn app() -> AppInfo {
    AppInfo {
        name: "Browser".into(),
        pid: ProcessId::new(1),
        process_instance: None,
        bundle_id: None,
        presentation: None,
    }
}

#[test]
fn evidence_carries_the_token_and_no_bounds() {
    let evidence = node_evidence(&node("button", "Go", Some("cdp:T:9")));
    assert_eq!(evidence.identifiers.retained_object(), Some("cdp:T:9"));
    assert_eq!(evidence.name, LocatorField::Known("Go".into()));
    assert!(matches!(evidence.ref_evidence.bounds, LocatorField::Absent));
}

#[test]
fn a_node_at_the_depth_limit_keeps_its_child_count_only() {
    let mut root = node("RootWebArea", "Docs", Some("cdp:T:1"));
    let mut group = node("group", "Box", Some("cdp:T:2"));
    group.children = vec![node("button", "Go", Some("cdp:T:3"))];
    root.children = vec![group];
    let app = app();
    let observation = ObservationRoot::Application {
        app: &app,
        surface: SnapshotSurface::Web,
    };
    let tree = observed_page(&observation, root, 1)
        .unwrap()
        .into_accessibility_tree()
        .unwrap();
    let group = &tree.children[0];
    assert!(group.children.is_empty());
    assert_eq!(group.children_count, Some(1));
}
