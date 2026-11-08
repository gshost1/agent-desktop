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

/// A complete search that found no candidate is stamped retryable by every
/// platform, because the element may be drawn again. The read-only wait honours
/// that stamp like `wait --element` and the dispatch-time retry do: it polls out
/// the budget and only then reports the last complete answer.
#[test]
fn expand_complete_no_candidate_polls_out_the_budget_then_reports_stale_ref() {
    let adapter = CompleteNoCandidateAdapter {
        resolve_calls: AtomicU32::new(0),
    };
    let error = execute_with_auto_wait(
        RefActionWaitCtx {
            adapter: &adapter,
            entry: &entry(),
            ref_id: "@e1",
            context: &CommandContext::default(),
        },
        ActionRequest::headless(Action::Expand).with_timeout_ms(Some(400)),
        crate::ref_action::ref_action_single::dispatch_resolved,
    )
    .unwrap_err();

    assert_eq!(error.code, ErrorCode::StaleRef);
    let details = error.details.expect("the last complete answer");
    assert_eq!(details["kind"], "resolve_no_candidate");
    assert!(details.get("elapsed_ms").is_some(), "{details}");
    assert!(
        adapter.resolve_calls.load(Ordering::SeqCst) > 2,
        "a retryable complete absence is polled again while budget remains"
    );
}

#[test]
fn a_retryable_complete_absence_that_clears_within_the_budget_lands() {
    let adapter = SequencedAdapter {
        resolve_calls: AtomicU32::new(0),
        failures: 2,
        retryable: true,
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
    .expect("the element was drawn again inside the budget");

    assert_eq!(result.action, "click");
    assert!(adapter.resolve_calls.load(Ordering::SeqCst) > 2);
}

#[test]
fn a_settled_complete_absence_still_stops_at_once() {
    let adapter = SequencedAdapter {
        resolve_calls: AtomicU32::new(0),
        failures: u32::MAX,
        retryable: false,
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

struct SequencedAdapter {
    resolve_calls: AtomicU32,
    failures: u32,
    retryable: bool,
}

impl ObservationOps for SequencedAdapter {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        let call = self.resolve_calls.fetch_add(1, Ordering::SeqCst);
        if call < self.failures {
            return Err(
                AdapterError::stale_ref_because("No candidate").with_details(serde_json::json!({
                    "kind": if self.retryable { "resolve_no_candidate" } else { "resolve_gone" },
                    "complete": true,
                    "retryable": self.retryable,
                })),
            );
        }
        Ok(NativeHandle::null())
    }

    crate::adapter::complete_live_observation!("button", "Run", [capability::CLICK]);
    crate::adapter::saved_ref_scope!("button", Some("Run"));
}

impl ActionOps for SequencedAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
        _lease: &crate::InteractionLease,
    ) -> Result<crate::actions::action_result::ActionResult, AdapterError> {
        Ok(crate::actions::action_result::ActionResult::delivered_unverified("click"))
    }
}

impl InputOps for SequencedAdapter {}

impl SystemOps for SequencedAdapter {
    crate::adapter::guarded_interaction_lease!();
}

/// The recorded path holds a same-role sibling for the first two element reads,
/// as a list re-sorting under the descent would, then the recorded element.
struct ReorderingAdapter {
    element_reads: AtomicU32,
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
        if self.element_reads.fetch_add(1, Ordering::SeqCst) < 2 {
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
