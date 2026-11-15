use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps, WindowFilter};
use crate::commands::wait::{WaitArgs, WaitModeArgs, WaitPredicateArgs};
use crate::locator::live_locator::{
    LocatorField, LocatorResolutionMeta, LocatorStats, ObservationRoot,
};
use crate::{AccessibilityNode, AdapterError, WindowInfo};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Label {
    Name,
    Value,
    Description,
}

/// A window whose label appears at read `ready_at`, nested under `wrappers`
/// unnamed groups. Like the macOS walk, it counts an unnamed group as free
/// depth only when the request asks for actions, cuts the tree at the
/// requested logical depth without recording a limit, and, when `honors_plan`
/// is set, leaves states unread unless asked for them.
struct LabelAdapter {
    ready_at: usize,
    wrappers: u8,
    label: Label,
    honors_plan: bool,
    reads: AtomicUsize,
    snapshot_reads: AtomicUsize,
    probe_depths: Mutex<Vec<(u8, u8)>>,
}

fn adapter(ready_at: usize, wrappers: u8, label: Label) -> LabelAdapter {
    LabelAdapter {
        ready_at,
        wrappers,
        label,
        honors_plan: true,
        reads: AtomicUsize::new(0),
        snapshot_reads: AtomicUsize::new(0),
        probe_depths: Mutex::new(Vec::new()),
    }
}

fn node(role: &str, name: Option<&str>, children: Vec<AccessibilityNode>) -> AccessibilityNode {
    AccessibilityNode {
        ref_id: None,
        role: role.into(),
        identity: crate::NodeIdentity {
            retained_object: None,
            name: name.map(Into::into),
            ..Default::default()
        },
        presentation: Default::default(),
        children_count: None,
        subtree_truncated: false,
        children,
    }
}

fn label(kind: Label, ready: bool) -> AccessibilityNode {
    let text = if ready { "Ready" } else { "Loading" };
    let mut label = node("statictext", None, Vec::new());
    match kind {
        Label::Name => label.identity.name = Some(text.into()),
        Label::Value => label.identity.value = Some(text.into()),
        Label::Description => label.identity.description = Some(text.into()),
    }
    label
}

impl LabelAdapter {
    fn chain(
        &self,
        depth: u8,
        logical: u8,
        request: &ObservationRequest,
        ready: bool,
    ) -> AccessibilityNode {
        if depth == self.wrappers {
            return label(self.label, ready);
        }
        let wrapper_is_free = request.descendant_evidence().ref_evidence.actions;
        let next = logical + u8::from(!wrapper_is_free);
        let children = if next > request.max_logical_depth {
            Vec::new()
        } else {
            vec![self.chain(depth + 1, next, request, ready)]
        };
        node("group", None, children)
    }
}

impl ObservationOps for LabelAdapter {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<crate::locator::live_locator::ObservedTree, AdapterError> {
        let read = self.reads.fetch_add(1, Ordering::SeqCst) + 1;
        let probe = !request.descendant_evidence().states;
        if probe {
            self.probe_depths
                .lock()
                .unwrap()
                .push((request.max_logical_depth, request.max_raw_depth));
        } else {
            self.snapshot_reads.fetch_add(1, Ordering::SeqCst);
        }
        let body = self.chain(0, 1, request, read >= self.ready_at);
        let mut tree =
            crate::adapter::observed_tree(&root, node("window", Some("Doc"), vec![body]))?;
        if probe && self.honors_plan {
            for observed in &mut tree.nodes {
                observed.evidence.states = LocatorField::Unknown;
            }
        }
        Ok(tree)
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

impl ActionOps for LabelAdapter {}
impl InputOps for LabelAdapter {}
impl SystemOps for LabelAdapter {}

fn wait_for(adapter: &LabelAdapter, timeout_ms: u64) -> Result<serde_json::Value, crate::AppError> {
    crate::commands::wait::execute(
        WaitArgs {
            mode: WaitModeArgs {
                text: Some("Ready".into()),
                ..WaitModeArgs::default()
            },
            predicate: WaitPredicateArgs {
                predicate: None,
                value: None,
                action: None,
                count: None,
            },
            timeout_ms,
            app: Some("TestApp".into()),
        },
        adapter,
        &crate::context::CommandContext::default(),
    )
}

/// A poll after the first reads the window for the text alone and builds a
/// snapshot only when that read cannot rule the text out, and the read goes
/// as deep as a locator walk, never cut at the snapshot's logical depth.
#[test]
fn a_text_wait_builds_a_snapshot_only_on_the_first_poll_and_once_the_text_can_be_there() {
    let _guard = crate::refs::refs_test_support::HomeGuard::new();
    let adapter = adapter(4, 0, Label::Name);

    let result = wait_for(&adapter, 10_000).expect("the label appears before the timeout");

    assert_eq!(result["found"], true);
    assert_eq!(adapter.snapshot_reads.load(Ordering::SeqCst), 2);
    assert_eq!(*adapter.probe_depths.lock().unwrap(), [(50, 50); 3]);
}

#[test]
fn text_held_only_in_a_value_or_a_description_is_not_ruled_out() {
    for kind in [Label::Value, Label::Description] {
        let _guard = crate::refs::refs_test_support::HomeGuard::new();
        let adapter = adapter(3, 0, kind);

        let result = wait_for(&adapter, 5_000).expect("the label appears at read 3");

        assert_eq!(result["found"], true);
        assert_eq!(adapter.snapshot_reads.load(Ordering::SeqCst), 2);
    }
}

/// An adapter that reads every field whatever the request makes the probe as
/// costly as the snapshot, so after one probe the wait polls snapshots only.
#[test]
fn an_adapter_that_reads_every_field_gets_one_probe_and_then_snapshots() {
    let _guard = crate::refs::refs_test_support::HomeGuard::new();
    let adapter = LabelAdapter {
        honors_plan: false,
        ..adapter(5, 0, Label::Name)
    };

    let result = wait_for(&adapter, 10_000).expect("the label appears at read 5");

    assert_eq!(result["found"], true);
    assert_eq!(adapter.probe_depths.lock().unwrap().len(), 1);
    assert_eq!(adapter.snapshot_reads.load(Ordering::SeqCst), 4);
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
fn only_a_read_that_missed_nothing_but_the_raw_depth_cut_rules_the_text_out() {
    assert!(absent_from_the_read(&resolution(
        0,
        true,
        LocatorStats::default()
    )));
    assert!(!absent_from_the_read(&resolution(
        1,
        true,
        LocatorStats::default()
    )));

    let mut depth_cut = LocatorStats::default();
    depth_cut.traversal.limits.depth_hits = 3;
    assert!(absent_from_the_read(&resolution(
        0,
        false,
        depth_cut.clone()
    )));

    let mut node_cut = depth_cut.clone();
    node_cut.traversal.limits.node_hits = 1;
    let mut label_cut = depth_cut.clone();
    label_cut.traversal.limits.child_label_hits = 1;
    let mut failed_read = depth_cut.clone();
    failed_read.reads.health.native_read_failures = 1;
    let mut deadline_cut = depth_cut;
    deadline_cut.reads.health.deadline_exhausted = 1;
    for stats in [node_cut, label_cut, failed_read, deadline_cut] {
        assert!(!absent_from_the_read(&resolution(0, false, stats)));
    }
    assert!(
        !absent_from_the_read(&resolution(0, false, LocatorStats::default())),
        "an incomplete read with no recorded cause cannot rule the text out"
    );
}
