use super::{DeferredList, LocatorTraversal, defers_children};
use crate::tree::query::child_read::ChildRead;
use agent_desktop_core::ObservedSubtree;

#[test]
fn row_containers_are_read_last_and_everything_else_in_place() {
    for role in ["AXOutline", "AXTable", "AXBrowser", "AXList"] {
        assert!(defers_children(Some(role)), "{role} rows must be read last");
    }
    for role in [
        "AXGroup",
        "AXSheet",
        "AXScrollArea",
        "AXRow",
        "AXPopUpButton",
    ] {
        assert!(!defers_children(Some(role)), "{role} must be read in place");
    }
    assert!(!defers_children(None));
}

fn traversal(leave_rows_unread: bool) -> LocatorTraversal {
    let mut request = agent_desktop_core::ObservationRequest::snapshot(
        &agent_desktop_core::TreeOptions::default(),
        agent_desktop_core::Deadline::after(1_000).unwrap(),
    );
    request.observation_mode.leave_rows_unread = leave_rows_unread;
    LocatorTraversal::new(
        &request,
        crate::tree::TreeBuildContext::empty(false),
        std::time::Instant::now() + std::time::Duration::from_secs(1),
    )
}

fn list_of_unread_rows() -> DeferredList {
    let mut rows = ChildRead::empty(true);
    rows.total_count = 3000;
    DeferredList {
        path: Vec::new(),
        children: rows,
        depths: (1, 0),
        viewport: None,
        ancestors: Vec::new(),
    }
}

#[test]
fn a_rowless_pass_leaves_rows_unread_without_failing_the_rest_of_the_walk() {
    let mut walk = traversal(true);
    walk.deferred.push_back(list_of_unread_rows());
    let list = ObservedSubtree::new(
        super::super::node_evidence::unknown(),
        Vec::new(),
        true,
        None,
    );

    let list = walk.read_deferred_lists(Some(list)).unwrap().unwrap();

    assert!(
        !list.is_complete(),
        "a list whose rows were not read is never complete"
    );
    assert!(
        walk.arena.structurally_complete,
        "rows left unread on request must not fail the walk around them"
    );
    assert_eq!(walk.arena.stats.reads.health.deadline_exhausted, 0);
    assert_eq!(walk.arena.stats.traversal.nodes_visited, 0);
}
