use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps, WindowFilter};
use crate::commands::wait::{WaitArgs, WaitModeArgs, WaitPredicateArgs};
use crate::locator::live_locator::{LocatorResolutionMeta, LocatorStats, ObservationRoot};
use crate::{AccessibilityNode, AdapterError, WindowInfo};
use std::sync::atomic::{AtomicUsize, Ordering};

const READY_AT_READ: usize = 4;

/// A window whose "Ready" label appears at its fourth read. Snapshot reads
/// ask for states; the text probe does not.
#[derive(Default)]
struct LateLabelAdapter {
    reads: AtomicUsize,
    snapshot_reads: AtomicUsize,
    probe_reads: AtomicUsize,
}

fn named(role: &str, name: &str, children: Vec<AccessibilityNode>) -> AccessibilityNode {
    AccessibilityNode {
        ref_id: None,
        role: role.into(),
        identity: crate::NodeIdentity {
            retained_object: None,
            name: Some(name.into()),
            ..Default::default()
        },
        presentation: Default::default(),
        children_count: None,
        subtree_truncated: false,
        children,
    }
}

impl ObservationOps for LateLabelAdapter {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<crate::locator::live_locator::ObservedTree, AdapterError> {
        let read = self.reads.fetch_add(1, Ordering::SeqCst) + 1;
        if request.descendant_evidence().states {
            self.snapshot_reads.fetch_add(1, Ordering::SeqCst);
        } else {
            self.probe_reads.fetch_add(1, Ordering::SeqCst);
        }
        let children = if read >= READY_AT_READ {
            vec![named("statictext", "Ready", Vec::new())]
        } else {
            vec![named("statictext", "Loading", Vec::new())]
        };
        crate::adapter::observed_tree(&root, named("window", "Doc", children))
    }

    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(vec![WindowInfo {
            id: "w-1".into(),
            title: "Doc".into(),
            app: "TestApp".into(),
            pid: crate::ProcessId::new(1),
            process_instance: Some("test-instance".into()),
            bounds: None,
            state: crate::WindowState {
                is_focused: true,
                ..Default::default()
            },
        }])
    }
}

impl ActionOps for LateLabelAdapter {}
impl InputOps for LateLabelAdapter {}
impl SystemOps for LateLabelAdapter {}

fn text_wait(text: &str) -> WaitArgs {
    WaitArgs {
        mode: WaitModeArgs {
            text: Some(text.into()),
            ..WaitModeArgs::default()
        },
        predicate: WaitPredicateArgs {
            predicate: None,
            value: None,
            action: None,
            count: None,
        },
        timeout_ms: 10_000,
        app: Some("TestApp".into()),
    }
}

/// A poll after the first reads the window for the text alone and builds a
/// snapshot only when that read cannot rule the text out. Without the probe
/// every poll built a snapshot, which reads every node's actions and states.
#[test]
fn a_text_wait_builds_a_snapshot_only_on_the_first_poll_and_once_the_text_can_be_there() {
    let _guard = crate::refs::refs_test_support::HomeGuard::new();
    let adapter = LateLabelAdapter::default();

    let result = crate::commands::wait::execute(
        text_wait("Ready"),
        &adapter,
        &crate::context::CommandContext::default(),
    )
    .expect("the label appears before the timeout");

    assert_eq!(result["found"], true);
    assert_eq!(
        adapter.snapshot_reads.load(Ordering::SeqCst),
        2,
        "one snapshot for the first poll and one for the poll whose probe saw the text"
    );
    assert_eq!(adapter.probe_reads.load(Ordering::SeqCst), 3);
}

fn resolution(total_matches: u32, complete: bool, stats: LocatorStats) -> LocatorResolution {
    LocatorResolution {
        matches: Vec::new(),
        refmap: None,
        stats,
        meta: LocatorResolutionMeta {
            total_matches,
            complete,
            selection_complete: complete,
            truncated: false,
            roles_present: Vec::new(),
        },
    }
}

#[test]
fn only_a_read_that_missed_nothing_but_the_depth_cut_rules_the_text_out() {
    assert!(absent_within_depth(&resolution(
        0,
        true,
        LocatorStats::default()
    )));
    assert!(!absent_within_depth(&resolution(
        1,
        true,
        LocatorStats::default()
    )));

    let mut depth_cut = LocatorStats::default();
    depth_cut.traversal.limits.depth_hits = 3;
    assert!(absent_within_depth(&resolution(
        0,
        false,
        depth_cut.clone()
    )));

    let mut node_cut = depth_cut.clone();
    node_cut.traversal.limits.node_hits = 1;
    assert!(!absent_within_depth(&resolution(0, false, node_cut)));

    let mut deadline_cut = depth_cut;
    deadline_cut.reads.health.deadline_exhausted = 1;
    assert!(!absent_within_depth(&resolution(0, false, deadline_cut)));

    assert!(
        !absent_within_depth(&resolution(0, false, LocatorStats::default())),
        "an incomplete read with no recorded cause cannot rule the text out"
    );
}
