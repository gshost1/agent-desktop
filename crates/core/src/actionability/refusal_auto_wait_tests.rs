use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use crate::{
    Action, ActionRequest, AdapterError, ErrorCode, RefEntry,
    actions::capability,
    adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps},
    context::CommandContext,
    ref_action::ref_action_wait::execute_with_auto_wait,
    ref_action::ref_action_wait_context::RefActionWaitContext,
    wait::deadline::VirtualClock,
};

const BOUNDS: crate::Rect = crate::Rect {
    x: 0.0,
    y: 0.0,
    width: 40.0,
    height: 20.0,
};

struct DisabledButton {
    started: Instant,
    enabled_after: Option<Duration>,
    dispatches: AtomicU32,
}

impl DisabledButton {
    fn new(enabled_after: Option<Duration>) -> Self {
        Self {
            started: crate::wait::deadline::now(),
            enabled_after,
            dispatches: AtomicU32::new(0),
        }
    }
}

impl ObservationOps for DisabledButton {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn get_live_element(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<crate::LiveElement, AdapterError> {
        let elapsed = crate::wait::deadline::now().saturating_duration_since(self.started);
        Ok(crate::LiveElement {
            identity: crate::adapter::live_identity("Run"),
            state: crate::ElementState {
                role: "button".into(),
                states: Vec::new(),
                value: None,
                enabled: Some(self.enabled_after.is_some_and(|after| elapsed >= after)),
                hidden: Some(false),
                offscreen: Some(false),
            },
            states_complete: true,
            bounds: Some(BOUNDS),
            available_actions: vec![capability::CLICK.into()],
        })
    }

    crate::adapter::saved_ref_scope!("button", Some("Run"));
}

impl ActionOps for DisabledButton {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
        _lease: &crate::InteractionLease,
    ) -> Result<crate::actions::action_result::ActionResult, AdapterError> {
        self.dispatches.fetch_add(1, Ordering::SeqCst);
        Ok(crate::actions::action_result::ActionResult::delivered_unverified("click"))
    }
}

impl InputOps for DisabledButton {}

impl SystemOps for DisabledButton {
    crate::adapter::guarded_interaction_lease!();
}

fn entry() -> RefEntry {
    let mut entry = crate::adapter::minimal_ref_entry("button", Some("Run"));
    entry.geometry.bounds = Some(BOUNDS);
    entry.geometry.bounds_hash = BOUNDS.bounds_hash();
    entry.capabilities.available_actions = vec![capability::CLICK.into()];
    entry
}

fn click(
    adapter: &DisabledButton,
    timeout_ms: u64,
) -> Result<crate::actions::action_result::ActionResult, AdapterError> {
    execute_with_auto_wait(
        RefActionWaitContext {
            adapter,
            entry: &entry(),
            ref_id: "@e1",
            context: &CommandContext::default(),
        },
        ActionRequest::headless(Action::Click).with_timeout_ms(Some(timeout_ms)),
        crate::ref_action::ref_action_single::dispatch_resolved,
    )
}

#[test]
fn a_click_on_the_default_budget_ends_a_disabled_target_at_the_settle_time() {
    let clock = VirtualClock::freeze();
    let adapter = DisabledButton::new(None);

    let error = click(&adapter, crate::DEFAULT_OPERATION_TIMEOUT_MS).unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionFailed);
    let details = error.details.clone().unwrap();
    assert_eq!(details["kind"], "target_disabled");
    assert_eq!(details["retryable"], false);
    assert!(
        clock.elapsed() >= super::DISABLED_SETTLE,
        "{:?}",
        clock.elapsed()
    );
    assert!(
        clock.elapsed() < Duration::from_millis(3_000),
        "{:?}",
        clock.elapsed()
    );
    assert_eq!(adapter.dispatches.load(Ordering::SeqCst), 0);
}

#[test]
fn a_click_with_a_budget_the_caller_chose_waits_it_out_and_times_out() {
    let clock = VirtualClock::freeze();
    let adapter = DisabledButton::new(None);

    let error = click(&adapter, 30_000).unwrap_err();

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.details.unwrap()["kind"], "actionability_timeout");
    assert!(
        clock.elapsed() >= Duration::from_millis(30_000),
        "{:?}",
        clock.elapsed()
    );
    assert_eq!(adapter.dispatches.load(Ordering::SeqCst), 0);
}

#[test]
fn a_target_enabled_inside_the_settle_time_is_clicked_once() {
    let _clock = VirtualClock::freeze();
    let adapter = DisabledButton::new(Some(Duration::from_millis(1_500)));

    let result = click(&adapter, crate::DEFAULT_OPERATION_TIMEOUT_MS);

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(adapter.dispatches.load(Ordering::SeqCst), 1);
}
