use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps, WindowFilter};
use crate::locator::live_locator::{ObservationRequest, ObservationRoot, ObservedTree};
use crate::refs::refs_test_support::HomeGuard;
use std::time::Duration;

/// Reads one window, spending 31 ms of virtual time inside the read so a 30 ms
/// inherited deadline expires after the tree is observed and before it is saved.
struct SlowSnapshotAdapter {
    truncated: bool,
}

const PARTIAL: SlowSnapshotAdapter = SlowSnapshotAdapter { truncated: true };
const COMPLETE: SlowSnapshotAdapter = SlowSnapshotAdapter { truncated: false };

impl ObservationOps for SlowSnapshotAdapter {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        _request: &ObservationRequest,
    ) -> Result<ObservedTree, crate::AdapterError> {
        crate::wait::deadline::sleep(Duration::from_millis(31));
        let tree = crate::AccessibilityNode {
            ref_id: None,
            role: "window".into(),
            identity: Default::default(),
            presentation: Default::default(),
            children_count: None,
            subtree_truncated: self.truncated,
            children: Vec::new(),
        };
        crate::adapter::observed_tree(&root, tree)
    }

    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<crate::WindowInfo>, crate::AdapterError> {
        Ok(vec![crate::WindowInfo {
            id: "w-1".into(),
            title: "Test".into(),
            app: "TestApp".into(),
            pid: crate::ProcessId::new(42),
            process_instance: Some("test-instance".into()),
            bounds: None,
            state: Default::default(),
        }])
    }
}

impl ActionOps for SlowSnapshotAdapter {}
impl InputOps for SlowSnapshotAdapter {}
impl SystemOps for SlowSnapshotAdapter {}

fn snapshot_test_app(
    adapter: &SlowSnapshotAdapter,
    timeout_ms: u64,
) -> Result<SnapshotResult, AppError> {
    run_with_context(
        adapter,
        &TreeOptions::default(),
        &SnapshotTarget {
            app_name: Some("TestApp"),
            window_id: None,
        },
        &crate::CommandContext::default(),
        timeout_ms,
    )
}

#[test]
fn expired_snapshot_deadline_persists_partial_tree_and_releases_the_store_lock() {
    let _home = HomeGuard::new();
    let _clock = crate::wait::deadline::VirtualClock::freeze();
    let inherited = crate::Deadline::detached_after(30).unwrap();
    let deadline_scope = crate::wait::deadline::enter_scope(Some(inherited));

    let result = snapshot_test_app(&PARTIAL, 30)
        .expect("a partial snapshot should persist after its observation deadline");

    assert!(!result.complete);
    let snapshot_id = result.snapshot_id.expect("snapshot id");
    drop(deadline_scope);
    let store = RefStore::new().unwrap();
    assert_eq!(store.latest_snapshot_id().unwrap(), Some(snapshot_id));

    let next = snapshot_test_app(&PARTIAL, 1_000)
        .expect("a later snapshot should acquire the released ref-store lock");
    assert!(next.snapshot_id.is_some());
}

#[test]
fn complete_snapshot_built_before_an_inherited_deadline_expires_is_still_persisted() {
    let _home = HomeGuard::new();
    let _clock = crate::wait::deadline::VirtualClock::freeze();
    let inherited = crate::Deadline::detached_after(30).unwrap();
    let deadline_scope = crate::wait::deadline::enter_scope(Some(inherited));

    let result = snapshot_test_app(&COMPLETE, 30)
        .expect("a complete snapshot observed in budget must not be discarded at save time");

    assert!(result.complete);
    assert!(inherited.is_expired());
    let snapshot_id = result.snapshot_id.expect("snapshot id");
    drop(deadline_scope);
    let store = RefStore::new().unwrap();
    assert_eq!(
        store.latest_snapshot_id().unwrap(),
        Some(snapshot_id.clone())
    );
    assert!(store.load_snapshot(&snapshot_id).is_ok());
}

#[test]
fn a_save_past_an_inherited_deadline_waits_at_most_the_lock_budget() {
    let _home = HomeGuard::new();
    let store = RefStore::new().unwrap();
    let held =
        crate::refs::refs_lock::RefStoreLock::acquire(&store.base_dir().join("refstore.lock"))
            .unwrap();
    let clock = crate::wait::deadline::VirtualClock::freeze();
    let inherited = crate::Deadline::detached_after(30).unwrap();
    let deadline_scope = crate::wait::deadline::enter_scope(Some(inherited));
    let options = TreeOptions::default();
    let mut result = build(&COMPLETE, &options, Some("TestApp"), None, inherited).unwrap();
    assert!(result.complete && inherited.is_expired());

    let save_started = clock.elapsed();
    let Err(AppError::Adapter(error)) = result.save_refmap(&store) else {
        panic!("a contended save must fail with an adapter error");
    };
    let waited = clock.elapsed() - save_started;
    drop(deadline_scope);

    assert_eq!(error.code, crate::ErrorCode::Timeout);
    assert_eq!(error.details.unwrap()["kind"], "lock_timeout");
    let lock_budget = Duration::from_millis(crate::refs::refs_lock::LOCK_TIMEOUT_MS);
    assert!(
        waited >= lock_budget && waited <= lock_budget + Duration::from_millis(10),
        "a save waits its own lock budget once, whatever the command had left: {waited:?}"
    );
    drop(held);
    assert!(store.latest_snapshot_id().unwrap().is_none());
}
