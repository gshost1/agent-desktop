use super::test_support::{snapshot_with_one_ref, wait_for_element_test};
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::locator::live_locator::{ObservationRequest, ObservationRoot, ObservedTree};
use crate::{
    AdapterError, adapter::NativeHandle, commands::wait_predicate, refs::RefEntry,
    refs::refs_test_support::HomeGuard,
};
use std::sync::atomic::{AtomicU32, Ordering};

struct LandingAdapter {
    window_reads: AtomicU32,
    value_reads: AtomicU32,
    ready_from_read: u32,
    twin_after_first_window: bool,
}

impl LandingAdapter {
    fn new(ready_from_read: u32, twin_after_first_window: bool) -> Self {
        Self {
            window_reads: AtomicU32::new(0),
            value_reads: AtomicU32::new(0),
            ready_from_read,
            twin_after_first_window,
        }
    }
}

impl ObservationOps for LandingAdapter {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        _request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        if matches!(root, ObservationRoot::Element { .. }) {
            return crate::adapter::saved_ref_element_tree(root);
        }
        let read = self.window_reads.fetch_add(1, Ordering::SeqCst) + 1;
        if self.twin_after_first_window && read > 1 {
            return crate::adapter::saved_ref_tree_nodes(
                root,
                &[("button", Some("Run"), None), ("button", Some("Run"), None)],
            );
        }
        crate::adapter::saved_ref_tree(root, "button", Some("Run"))
    }

    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn get_live_value(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<Option<String>, AdapterError> {
        let read = self.value_reads.fetch_add(1, Ordering::SeqCst) + 1;
        let value = if read >= self.ready_from_read {
            "ready"
        } else {
            "busy"
        };
        Ok(Some(value.into()))
    }

    crate::adapter::saved_ref_anchor!();
    crate::adapter::saved_ref_scope!(@inventory);
}

impl ActionOps for LandingAdapter {}
impl InputOps for LandingAdapter {}
impl SystemOps for LandingAdapter {}

fn wait_for_ready(adapter: &LandingAdapter, timeout_ms: u64) -> Result<serde_json::Value, crate::AppError> {
    wait_for_element_test(
        "@e1".into(),
        Some(snapshot_with_one_ref()),
        wait_predicate::ElementPredicate::Value("ready".into()),
        timeout_ms,
        adapter,
        &crate::context::CommandContext::default(),
    )
}

#[test]
fn an_unmet_predicate_polls_by_landing_and_counts_the_window_once_more_to_report() {
    let _guard = HomeGuard::new();
    let adapter = LandingAdapter::new(4, false);

    let result = wait_for_ready(&adapter, 3_000).unwrap();

    assert_eq!(result["found"], true);
    assert_eq!(adapter.value_reads.load(Ordering::SeqCst), 5);
    assert_eq!(adapter.window_reads.load(Ordering::SeqCst), 2);
}

#[test]
fn a_landed_match_is_not_reported_while_the_window_holds_a_twin() {
    let _guard = HomeGuard::new();
    let adapter = LandingAdapter::new(3, true);

    let error = wait_for_ready(&adapter, 600).unwrap_err();

    assert!(
        matches!(error.code(), "AMBIGUOUS_TARGET" | "TIMEOUT"),
        "a twin must refuse the wait, got {}",
        error.code()
    );
    assert!(adapter.window_reads.load(Ordering::SeqCst) >= 2);
}
