use super::*;

struct CompleteNoCandidateAdapter {
    resolve_calls: AtomicU32,
}

impl ObservationOps for CompleteNoCandidateAdapter {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        self.resolve_calls.fetch_add(1, Ordering::SeqCst);
        Err(
            AdapterError::stale_ref_because("No candidate").with_details(serde_json::json!({
                "kind": "resolve_no_candidate",
                "complete": true,
                "retryable": true,
            })),
        )
    }

    crate::adapter::complete_live_observation!("menubutton", "Advanced", [capability::EXPAND]);
    crate::adapter::saved_ref_scope!("button", Some("Run"));
}

impl ActionOps for CompleteNoCandidateAdapter {}
impl InputOps for CompleteNoCandidateAdapter {}
impl SystemOps for CompleteNoCandidateAdapter {}

#[test]
fn expand_complete_no_candidate_fails_immediately_with_stale_ref() {
    let adapter = CompleteNoCandidateAdapter {
        resolve_calls: AtomicU32::new(0),
    };
    let started = std::time::Instant::now();
    let error = execute_with_auto_wait(
        RefActionWaitCtx {
            adapter: &adapter,
            entry: &entry(),
            ref_id: "@e1",
            context: &CommandContext::default(),
        },
        ActionRequest::headless(Action::Expand).with_timeout_ms(Some(5_000)),
        crate::ref_action::ref_action_single::dispatch_resolved,
    )
    .unwrap_err();

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(
        adapter.resolve_calls.load(Ordering::SeqCst),
        2,
        "one provisional landing and one full resolution, then no retry"
    );
    assert!(started.elapsed() < std::time::Duration::from_millis(500));
}

#[test]
fn a_settled_complete_absence_still_stops_at_once() {
    let adapter = SettledAbsenceAdapter {
        resolve_calls: AtomicU32::new(0),
    };
    let started = std::time::Instant::now();
    let error = execute_with_auto_wait(
        RefActionWaitCtx {
            adapter: &adapter,
            entry: &entry(),
            ref_id: "@e1",
            context: &CommandContext::default(),
        },
        request_with_timeout(5_000),
        crate::ref_action::ref_action_single::dispatch_resolved,
    )
    .unwrap_err();

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(error.details.expect("details")["kind"], "resolve_gone");
    assert_eq!(
        adapter.resolve_calls.load(Ordering::SeqCst),
        2,
        "one provisional landing and one full resolution, then no retry"
    );
    assert!(started.elapsed() < std::time::Duration::from_millis(500));
}

/// A list that reorders between the whole-window count and the descent lands a
/// same-role sibling. The element still exists, so the wait retries, and the
/// next attempt lands it.
#[test]
fn a_landing_raced_by_a_reorder_retries_and_then_lands() {
    let adapter = ReorderingAdapter {
        element_reads: AtomicU32::new(0),
        moved_reads: 2,
    };
    let result = execute_with_auto_wait(
        RefActionWaitCtx {
            adapter: &adapter,
            entry: &entry(),
            ref_id: "@e1",
            context: &CommandContext::default(),
        },
        request_with_timeout(5_000),
        crate::ref_action::ref_action_single::dispatch_resolved,
    )
    .expect("the reordered element lands on the next attempt");

    assert_eq!(result.action, "click");
    assert!(adapter.element_reads.load(Ordering::SeqCst) > 2);
}

#[test]
fn incomplete_resolution_still_polls_until_success() {
    let adapter = RetryAdapter {
        resolve_calls: AtomicU32::new(0),
    };
    let result = execute_with_auto_wait(
        RefActionWaitCtx {
            adapter: &adapter,
            entry: &entry(),
            ref_id: "@e1",
            context: &CommandContext::default(),
        },
        request_with_timeout(5_000),
        crate::ref_action::ref_action_single::dispatch_resolved,
    )
    .unwrap();

    assert_eq!(result.action, "click");
    assert!(adapter.resolve_calls.load(Ordering::SeqCst) >= 3);
}

/// A path that keeps landing a different element is not a list that is
/// settling: after two retried moved landings the action stops with a
/// stale-ref diagnosis instead of a generic timeout at the end of its budget.
#[test]
fn a_landing_that_keeps_moving_stops_as_saved_ref_absent_after_two_retries() {
    let adapter = ReorderingAdapter {
        element_reads: AtomicU32::new(0),
        moved_reads: u32::MAX,
    };
    let started = std::time::Instant::now();
    let error = execute_with_auto_wait(
        RefActionWaitCtx {
            adapter: &adapter,
            entry: &entry(),
            ref_id: "@e1",
            context: &CommandContext::default(),
        },
        request_with_timeout(5_000),
        crate::ref_action::ref_action_single::dispatch_resolved,
    )
    .unwrap_err();

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(error.details.expect("details")["kind"], "saved_ref_absent");
    assert_eq!(
        adapter.element_reads.load(Ordering::SeqCst),
        6,
        "three attempts, each a provisional landing and a counted one"
    );
    assert!(started.elapsed() < std::time::Duration::from_millis(2_000));
}

struct SettledAbsenceAdapter {
    resolve_calls: AtomicU32,
}

impl ObservationOps for SettledAbsenceAdapter {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        self.resolve_calls.fetch_add(1, Ordering::SeqCst);
        Err(
            AdapterError::stale_ref_because("Gone").with_details(serde_json::json!({
                "kind": "resolve_gone",
                "complete": true,
                "retryable": false,
            })),
        )
    }

    crate::adapter::complete_live_observation!("button", "Run", [capability::CLICK]);
    crate::adapter::saved_ref_scope!("button", Some("Run"));
}

impl ActionOps for SettledAbsenceAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
        _lease: &crate::InteractionLease,
    ) -> Result<crate::actions::action_result::ActionResult, AdapterError> {
        Ok(crate::actions::action_result::ActionResult::delivered_unverified("click"))
    }
}

impl InputOps for SettledAbsenceAdapter {}

impl SystemOps for SettledAbsenceAdapter {
    crate::adapter::guarded_interaction_lease!();
}

/// The recorded path holds a same-role sibling for the first `moved_reads`
/// element reads, as a list re-sorting under the descent would, then the
/// recorded element.
struct ReorderingAdapter {
    element_reads: AtomicU32,
    moved_reads: u32,
}

impl ObservationOps for ReorderingAdapter {
    fn observe_tree(
        &self,
        root: crate::locator::live_locator::ObservationRoot<'_>,
        _request: &crate::locator::live_locator::ObservationRequest,
    ) -> Result<crate::locator::live_locator::ObservedTree, AdapterError> {
        if !matches!(
            root,
            crate::locator::live_locator::ObservationRoot::Element { .. }
        ) {
            return crate::adapter::saved_ref_tree(root, "button", Some("Run"));
        }
        if self.element_reads.fetch_add(1, Ordering::SeqCst) < self.moved_reads {
            return crate::adapter::observed_tree(
                &root,
                crate::adapter::minimal_accessibility_node("button", "Other"),
            );
        }
        crate::adapter::saved_ref_element_tree(root)
    }

    fn resolve_locator_anchor(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    crate::adapter::complete_live_observation!("button", "Run", [capability::CLICK]);
    crate::adapter::saved_ref_scope!(@inventory);
}

impl ActionOps for ReorderingAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
        _lease: &crate::InteractionLease,
    ) -> Result<crate::actions::action_result::ActionResult, AdapterError> {
        Ok(crate::actions::action_result::ActionResult::delivered_unverified("click"))
    }
}

impl InputOps for ReorderingAdapter {}

impl SystemOps for ReorderingAdapter {
    crate::adapter::guarded_interaction_lease!();
}
