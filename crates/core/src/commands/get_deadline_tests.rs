use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::locator::live_locator::{ObservationRequest, ObservationRoot, ObservedTree};
use crate::{
    AdapterError, adapter::NativeHandle, refs::RefMap, refs::refs_store::RefStore,
    refs::refs_test_support::HomeGuard,
};
use std::sync::Mutex;
use std::time::Duration;

const RESOLVE_COST: Duration = Duration::from_millis(4_000);

#[derive(Default)]
struct SlowWindowAdapter {
    read_budgets_ms: Mutex<Vec<u64>>,
}

impl ObservationOps for SlowWindowAdapter {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        _request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        if matches!(root, ObservationRoot::Window(_)) {
            crate::wait::deadline::sleep(RESOLVE_COST);
        }
        crate::adapter::saved_ref_tree(root, "textfield", Some("Search"))
    }

    fn resolve_element_strict(
        &self,
        _entry: &crate::RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn get_live_value(
        &self,
        _handle: &NativeHandle,
        deadline: crate::Deadline,
    ) -> Result<Option<String>, AdapterError> {
        self.read_budgets_ms
            .lock()
            .unwrap()
            .push(deadline.remaining_ms());
        Ok(Some("query".into()))
    }

    crate::adapter::saved_ref_anchor!();
    crate::adapter::saved_ref_scope!(@inventory);
}

impl ActionOps for SlowWindowAdapter {}
impl InputOps for SlowWindowAdapter {}
impl SystemOps for SlowWindowAdapter {}

#[test]
fn the_read_after_a_slow_resolution_gets_only_what_the_command_has_left() {
    let _guard = HomeGuard::new();
    let _clock = crate::wait::deadline::VirtualClock::freeze();
    let mut refmap = RefMap::new();
    refmap.allocate(crate::adapter::minimal_ref_entry("textfield", Some("Search")));
    let snapshot_id = RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap();
    let adapter = SlowWindowAdapter::default();

    let result = execute(
        GetArgs {
            ref_id: format!("@{snapshot_id}:e1"),
            property: GetProperty::Value,
        },
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(result["value"], "query");
    let budgets = adapter.read_budgets_ms.lock().unwrap().clone();
    assert_eq!(budgets.len(), 1);
    assert!(
        budgets[0] <= 1_000,
        "the read had {} ms after a 4 s resolution of a 5 s command",
        budgets[0]
    );
}
