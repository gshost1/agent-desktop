use crate::{
    Action, ActionOps, ActionRequest, ActionResult, AdapterError, Deadline, DeliverySemantics,
    ErrorCode, InputOps, InteractionLease, NativeHandle, ObservationOps, RefEntry, SystemOps,
    WindowInfo, actions::capability,
};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lease {
    Granted,
    TimesOut,
    TimesOutOnRetry,
    SpendsTheBudget,
}

struct PreDispatchAdapter {
    lease: Lease,
    focus_fails: bool,
    stale_once_under_lease: AtomicBool,
    lease_calls: AtomicU32,
    dispatches: AtomicU32,
}

impl PreDispatchAdapter {
    fn new(lease: Lease) -> Self {
        Self {
            lease,
            focus_fails: false,
            stale_once_under_lease: AtomicBool::new(false),
            lease_calls: AtomicU32::new(0),
            dispatches: AtomicU32::new(0),
        }
    }
}

impl ObservationOps for PreDispatchAdapter {
    fn resolve_element_strict(
        &self,
        _: &RefEntry,
        _: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        let leased = self.lease_calls.load(Ordering::SeqCst) == 1;
        if self.lease == Lease::TimesOutOnRetry
            && leased
            && !self.stale_once_under_lease.swap(true, Ordering::SeqCst)
        {
            return Err(
                AdapterError::new(ErrorCode::StaleRef, "moved under the lease")
                    .with_details(serde_json::json!({ "retryable": true })),
            );
        }
        Ok(NativeHandle::null())
    }

    crate::adapter::complete_live_observation!("button", "Run", [capability::CLICK]);
    crate::adapter::saved_ref_scope!("button", Some("Run"));
}

impl ActionOps for PreDispatchAdapter {
    fn execute_action(
        &self,
        _: &NativeHandle,
        _: ActionRequest,
        _: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        self.dispatches.fetch_add(1, Ordering::SeqCst);
        Ok(ActionResult::delivered_unverified("click"))
    }
}

impl InputOps for PreDispatchAdapter {}

impl SystemOps for PreDispatchAdapter {
    fn acquire_interaction_lease(
        &self,
        deadline: Deadline,
    ) -> Result<InteractionLease, AdapterError> {
        let call = self.lease_calls.fetch_add(1, Ordering::SeqCst) + 1;
        let lock_timeout = || {
            Err(deadline
                .timeout_error()
                .with_details(serde_json::json!({ "kind": "interaction_process_lock_timeout" })))
        };
        match self.lease {
            Lease::TimesOut => lock_timeout(),
            Lease::TimesOutOnRetry if call > 1 => lock_timeout(),
            Lease::SpendsTheBudget => {
                crate::wait::deadline::sleep(deadline.remaining());
                InteractionLease::guarded(deadline, ())
            }
            Lease::Granted | Lease::TimesOutOnRetry => InteractionLease::guarded(deadline, ()),
        }
    }

    fn resolve_window_strict(
        &self,
        window: &WindowInfo,
        _: Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        Ok(window.clone())
    }

    fn focus_window(&self, _: &WindowInfo, _: &InteractionLease) -> Result<(), AdapterError> {
        if self.focus_fails {
            return Err(AdapterError::timeout("x"));
        }
        Ok(())
    }
}

fn entry() -> RefEntry {
    let bounds = crate::Rect {
        x: 1.0,
        y: 1.0,
        width: 20.0,
        height: 20.0,
    };
    serde_json::from_value(serde_json::json!({
        "pid": 1, "process_instance": "test-instance", "role": "button", "name": "Run",
        "source_window_id": "w-1", "source_surface": "window", "path": [0],
        "unique_at_capture": true, "bounds": bounds, "bounds_hash": bounds.bounds_hash(),
        "available_actions": ["Click"], "states": []
    }))
    .unwrap()
}

fn click(adapter: &PreDispatchAdapter, request: ActionRequest) -> AdapterError {
    let _clock = crate::wait::deadline::VirtualClock::freeze();
    crate::ref_action::execute_entry(adapter, &entry(), request.with_timeout_ms(Some(3_000)))
        .unwrap_err()
}

#[test]
fn a_lease_timeout_is_not_delivered_and_is_not_retried() {
    let adapter = PreDispatchAdapter::new(Lease::TimesOut);

    let error = click(&adapter, ActionRequest::headless(Action::Click));

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(
        error.details.unwrap()["kind"],
        "interaction_process_lock_timeout"
    );
    assert_eq!(adapter.lease_calls.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.dispatches.load(Ordering::SeqCst), 0);
}

#[test]
fn a_lease_timeout_after_a_refusal_reports_that_refusal() {
    let adapter = PreDispatchAdapter::new(Lease::TimesOutOnRetry);

    let error = click(&adapter, ActionRequest::headless(Action::Click));

    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(error.details.unwrap()["refusal"]["code"], "STALE_REF");
    assert_eq!(adapter.lease_calls.load(Ordering::SeqCst), 2);
    assert_eq!(adapter.dispatches.load(Ordering::SeqCst), 0);
}

#[test]
fn a_budget_spent_while_taking_the_lease_is_not_delivered() {
    let adapter = PreDispatchAdapter::new(Lease::SpendsTheBudget);

    let error = click(&adapter, ActionRequest::headless(Action::Click));

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(adapter.lease_calls.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.dispatches.load(Ordering::SeqCst), 0);
}

#[test]
fn a_headed_focus_failure_is_not_delivered_and_records_the_focus() {
    let mut adapter = PreDispatchAdapter::new(Lease::Granted);
    adapter.focus_fails = true;

    let error = click(&adapter, ActionRequest::headed(Action::Click));

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(
        error.details.unwrap()["preparatory_focus"],
        "may_have_moved_focus"
    );
    assert_eq!(adapter.dispatches.load(Ordering::SeqCst), 0);
}

#[test]
fn a_headed_click_with_a_granted_lease_and_focus_is_dispatched_once() {
    let adapter = PreDispatchAdapter::new(Lease::Granted);
    let _clock = crate::wait::deadline::VirtualClock::freeze();

    crate::ref_action::execute_entry(
        &adapter,
        &entry(),
        ActionRequest::headed(Action::Click).with_timeout_ms(Some(3_000)),
    )
    .unwrap();

    assert_eq!(adapter.dispatches.load(Ordering::SeqCst), 1);
}

#[test]
fn a_pre_dispatch_refusal_without_evidence_becomes_not_delivered() {
    for step in [
        super::PreDispatch::Lease,
        super::PreDispatch::Focus,
        super::PreDispatch::Trace,
    ] {
        let error = step.refused(AdapterError::timeout("x"));
        assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    }
}

#[test]
fn a_pre_dispatch_refusal_keeps_stronger_evidence() {
    let error = super::PreDispatch::Focus
        .refused(AdapterError::timeout("x").with_disposition(DeliverySemantics::uncertain()));

    assert_eq!(error.disposition, DeliverySemantics::uncertain());
}

#[test]
fn a_focus_refusal_records_the_focus_beside_earlier_details() {
    let scrolled = super::PreDispatch::Focus.refused(AdapterError::timeout("x").with_details(
        serde_json::json!({
            "preparatory_scroll": "may_have_changed_view"
        }),
    ));
    let bare = super::PreDispatch::Focus
        .refused(AdapterError::timeout("x").with_details(serde_json::json!("cause")));

    let scrolled = scrolled.details.unwrap();
    assert_eq!(scrolled["preparatory_scroll"], "may_have_changed_view");
    assert_eq!(scrolled["preparatory_focus"], "may_have_moved_focus");
    let bare = bare.details.unwrap();
    assert_eq!(bare["error_details"], "cause");
    assert_eq!(bare["preparatory_focus"], "may_have_moved_focus");
}

#[test]
fn a_lease_or_trace_refusal_records_no_side_effect() {
    let lease = super::PreDispatch::Lease.refused(AdapterError::timeout("x"));
    let trace = super::PreDispatch::Trace.refused(crate::ref_action::into_adapter_error(
        crate::AppError::Internal("trace write failed".into()),
    ));

    assert!(lease.details.is_none());
    assert_eq!(trace.code, ErrorCode::Internal);
    assert_eq!(trace.disposition, DeliverySemantics::not_delivered());
    assert!(trace.details.is_none());
}
