use super::tests::{
    StubAdapter, drill_opts, load_latest, named, save_latest, seed_skeleton_refmap,
};
use super::*;
use crate::AdapterError;
use crate::adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps};
use crate::locator::live_locator::ObservedTree;
use crate::refs::refs_test_support::HomeGuard;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Spends 31 ms of virtual time in its first read, so a 30 ms inherited
/// deadline expires after the drill has observed its subtree and before the
/// refs it built are written back.
struct SlowDrillAdapter {
    inner: StubAdapter,
    slept: AtomicBool,
}

impl ObservationOps for SlowDrillAdapter {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        if !self.slept.swap(true, Ordering::SeqCst) {
            crate::wait::deadline::sleep(Duration::from_millis(31));
        }
        self.inner.observe_tree(root, request)
    }

    fn list_apps(&self, deadline: crate::Deadline) -> Result<Vec<crate::AppInfo>, AdapterError> {
        self.inner.list_apps(deadline)
    }

    fn list_windows(
        &self,
        filter: &crate::adapter::WindowFilter,
        deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        self.inner.list_windows(filter, deadline)
    }

    fn resolve_element_strict(
        &self,
        entry: &crate::refs::RefEntry,
        deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        self.inner.resolve_element_strict(entry, deadline)
    }
}

impl ActionOps for SlowDrillAdapter {}
impl InputOps for SlowDrillAdapter {}
impl SystemOps for SlowDrillAdapter {}

#[test]
fn a_drill_observed_before_an_inherited_deadline_expires_is_still_written_back() {
    let _guard = HomeGuard::new();
    let snapshot_id = save_latest(seed_skeleton_refmap());
    let mut subtree_root = named("group", "Sidebar");
    subtree_root.children = vec![named("button", "Save")];
    let adapter = SlowDrillAdapter {
        inner: StubAdapter::new(subtree_root),
        slept: AtomicBool::new(false),
    };
    let _clock = crate::wait::deadline::VirtualClock::freeze();
    let inherited = crate::Deadline::detached_after(30).unwrap();
    let deadline_scope = crate::wait::deadline::enter_scope(Some(inherited));

    let result = run_from_ref(&adapter, &drill_opts(), "@e1", Some(&snapshot_id))
        .expect("a drill observed in budget must not be discarded at save time");
    assert!(inherited.is_expired());
    drop(deadline_scope);

    assert!(result.refmap.len() >= 3);
    assert_eq!(load_latest().len(), result.refmap.len());
}
