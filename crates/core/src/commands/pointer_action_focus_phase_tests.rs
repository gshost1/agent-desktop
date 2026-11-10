use crate::adapter::hit_test::HitTestResult;
use crate::adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps};
use crate::commands::pointer_action::{point_failed_check, tests::point_ref_snapshot};
use crate::commands::{drag, hover};
use crate::refs::RefEntry;
use crate::refs::refs_test_support::HomeGuard;
use crate::{AdapterError, AppError, CommandContext, DragParams, ErrorCode, Rect};
use serde_json::json;
use std::sync::atomic::{AtomicU32, Ordering};

const TIMEOUT_MS: u64 = 400;
const FOREVER: u32 = u32::MAX;
const TARGET: Rect = Rect {
    x: 100.0,
    y: 200.0,
    width: 20.0,
    height: 10.0,
};

#[derive(Default)]
struct Counts {
    hit_tests: AtomicU32,
    focuses: AtomicU32,
    leases: AtomicU32,
}

struct CoveredTargetAdapter {
    clear_hit_tests: u32,
    occlusions: AtomicU32,
    counts: Counts,
    focus_fails_retryably: bool,
    offscreen_until_focus: bool,
}

impl CoveredTargetAdapter {
    fn new(occlusions: u32) -> Self {
        Self {
            clear_hit_tests: 0,
            occlusions: AtomicU32::new(occlusions),
            counts: Counts::default(),
            focus_fails_retryably: false,
            offscreen_until_focus: false,
        }
    }

    fn focuses(&self) -> u32 {
        self.counts.focuses.load(Ordering::SeqCst)
    }

    fn leases(&self) -> u32 {
        self.counts.leases.load(Ordering::SeqCst)
    }
}

impl ObservationOps for CoveredTargetAdapter {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn observe_pointer(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<crate::PointerObservation, AdapterError> {
        let states = if self.offscreen_until_focus && self.focuses() == 0 {
            vec![crate::tree::state::OFFSCREEN.to_string()]
        } else {
            Vec::new()
        };
        Ok(crate::PointerObservation {
            bounds: Some(TARGET),
            states,
        })
    }

    fn get_element_bounds(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<Option<Rect>, AdapterError> {
        Ok(Some(TARGET))
    }

    fn hit_test(
        &self,
        _handle: &NativeHandle,
        _point: crate::Point,
        _deadline: crate::Deadline,
    ) -> Result<HitTestResult, AdapterError> {
        let index = self.counts.hit_tests.fetch_add(1, Ordering::SeqCst);
        let occluded = index >= self.clear_hit_tests
            && self
                .occlusions
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                    if left == FOREVER {
                        Some(left)
                    } else {
                        left.checked_sub(1)
                    }
                })
                .is_ok();
        if occluded {
            return Ok(HitTestResult::InterceptedBy {
                role: Some("window".into()),
                name: Some("Covering window".into()),
                bounds: None,
            });
        }
        Ok(HitTestResult::ReachesTarget)
    }
    crate::adapter::saved_ref_scope!("button", Some("Target"));
}

impl ActionOps for CoveredTargetAdapter {
    fn scroll_into_view(
        &self,
        _handle: &NativeHandle,
        _lease: &crate::InteractionLease,
    ) -> Result<(), AdapterError> {
        Ok(())
    }
}

impl InputOps for CoveredTargetAdapter {
    fn mouse_event(
        &self,
        _event: crate::MouseEvent,
        _lease: &crate::InteractionLease,
    ) -> Result<crate::OperationReport, AdapterError> {
        Ok(crate::adapter::test_report())
    }

    fn drag(
        &self,
        _params: DragParams,
        _lease: &crate::InteractionLease,
    ) -> Result<crate::OperationReport, AdapterError> {
        Ok(crate::adapter::test_report())
    }
}

impl SystemOps for CoveredTargetAdapter {
    fn acquire_interaction_lease(
        &self,
        deadline: crate::Deadline,
    ) -> Result<crate::InteractionLease, AdapterError> {
        self.counts.leases.fetch_add(1, Ordering::SeqCst);
        crate::InteractionLease::guarded(deadline, ())
    }

    fn resolve_window_strict(
        &self,
        window: &crate::WindowInfo,
        _deadline: crate::Deadline,
    ) -> Result<crate::WindowInfo, AdapterError> {
        Ok(window.clone())
    }

    fn resolve_ref_focus_window(
        &self,
        entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<crate::WindowInfo, AdapterError> {
        crate::saved_ref_focus_window(entry)
    }

    fn focus_window(
        &self,
        _window: &crate::WindowInfo,
        _lease: &crate::InteractionLease,
    ) -> Result<(), AdapterError> {
        let focuses = self.counts.focuses.fetch_add(1, Ordering::SeqCst);
        if self.focus_fails_retryably && focuses == 0 {
            return Err(AdapterError::app_unresponsive("Fixture")
                .with_details(json!({ "retryable": true })));
        }
        Ok(())
    }
}

fn hover_args(snapshot_id: &str) -> hover::HoverArgs {
    hover::HoverArgs {
        ref_id: Some(format!("@{snapshot_id}:e1")),
        xy: None,
        duration_ms: None,
        timeout_ms: Some(TIMEOUT_MS),
    }
}

fn drag_args(snapshot_id: &str) -> drag::DragArgs {
    drag::DragArgs {
        from: drag::DragEndpoint {
            ref_id: Some(format!("@{snapshot_id}:e1")),
            xy: None,
        },
        to: drag::DragEndpoint {
            ref_id: Some(format!("@{snapshot_id}:e1")),
            xy: None,
        },
        duration_ms: None,
        drop_delay_ms: None,
        timeout_ms: Some(TIMEOUT_MS),
        wait_for_scope: drag::WaitForScope::default(),
    }
}

fn headed() -> CommandContext {
    CommandContext::default().with_headed(true)
}

fn moved_focus(error: &AppError) -> bool {
    match error {
        AppError::Adapter(error) => error.moved_focus(),
        _ => false,
    }
}

fn refusal(check: &str) -> AppError {
    AdapterError::new(ErrorCode::ActionFailed, "not actionable")
        .with_details(json!({ "check": check }))
        .into()
}

#[test]
fn a_covered_hover_raises_its_window_once_and_times_out_with_preparatory_focus() {
    let _guard = HomeGuard::new();
    let snapshot_id = point_ref_snapshot();
    let adapter = CoveredTargetAdapter::new(FOREVER);

    let error = hover::execute(hover_args(&snapshot_id), &adapter, &headed()).unwrap_err();

    assert_eq!(adapter.focuses(), 1);
    assert!(adapter.leases() >= 2, "the wait polled only once");
    assert_eq!(error.code(), "TIMEOUT");
    assert!(moved_focus(&error));
    let AppError::Adapter(error) = error else {
        panic!("expected an adapter error");
    };
    let refusal = &error.details.as_ref().unwrap()["refusal"];
    assert!(refusal.to_string().contains("receives_events"));
    assert!(
        error
            .suggestion
            .unwrap_or_default()
            .contains("covered the target")
    );
}

#[test]
fn a_transient_overlay_clears_without_a_second_raise() {
    let _guard = HomeGuard::new();
    let snapshot_id = point_ref_snapshot();
    let adapter = CoveredTargetAdapter::new(2);

    let value = hover::execute(hover_args(&snapshot_id), &adapter, &headed()).unwrap();

    assert_eq!(adapter.focuses(), 1);
    assert_eq!(adapter.leases(), 3);
    assert_eq!(value["focused"], true);
}

#[test]
fn a_drag_onto_a_covered_drop_point_raises_its_window_once() {
    let _guard = HomeGuard::new();
    let snapshot_id = point_ref_snapshot();
    let mut adapter = CoveredTargetAdapter::new(FOREVER);
    adapter.clear_hit_tests = 1;

    let error = drag::execute(drag_args(&snapshot_id), &adapter, &headed()).unwrap_err();

    assert_eq!(adapter.focuses(), 1);
    assert!(adapter.leases() >= 2, "the wait polled only once");
    assert_eq!(error.code(), "TIMEOUT");
    assert!(moved_focus(&error));
}

#[test]
fn a_raise_made_while_the_wait_scrolls_is_the_commands_only_raise() {
    let _guard = HomeGuard::new();
    let snapshot_id = point_ref_snapshot();
    let mut adapter = CoveredTargetAdapter::new(0);
    adapter.offscreen_until_focus = true;

    let value = hover::execute(hover_args(&snapshot_id), &adapter, &headed()).unwrap();

    assert_eq!(adapter.focuses(), 1);
    assert_eq!(value["focused"], true);
}

#[test]
fn a_focus_call_that_failed_after_it_may_have_raised_ends_the_command() {
    let _guard = HomeGuard::new();
    let snapshot_id = point_ref_snapshot();
    let mut adapter = CoveredTargetAdapter::new(0);
    adapter.focus_fails_retryably = true;

    let error = hover::execute(hover_args(&snapshot_id), &adapter, &headed()).unwrap_err();

    assert_eq!(adapter.focuses(), 1);
    assert_eq!(adapter.leases(), 1);
    assert_eq!(error.code(), "APP_UNRESPONSIVE");
    assert!(moved_focus(&error));
}

fn marked_phase<T>(
    timeout_ms: u64,
    raised: bool,
    mut attempt: impl FnMut() -> Result<T, AppError>,
) -> Result<T, AppError> {
    let deadline = crate::Deadline::after(timeout_ms.max(1)).unwrap();
    super::CommandFocus::default().marking(|focus| {
        focus.raised = raised;
        super::retry_leased_point_phase(Some(timeout_ms), deadline, focus, |_| attempt())
    })
}

#[test]
fn a_refusal_after_the_raise_carries_preparatory_focus() {
    let mut attempts = 0;

    let error = marked_phase::<()>(0, true, || {
        attempts += 1;
        Err(refusal("receives_events"))
    })
    .unwrap_err();

    assert_eq!(attempts, 1);
    assert!(point_failed_check(&error, "receives_events"));
    assert!(moved_focus(&error));
}

#[test]
fn every_error_after_the_raise_carries_preparatory_focus_and_keeps_its_code() {
    let error = marked_phase::<()>(TIMEOUT_MS, true, || {
        Err(AppError::Internal("trace sink closed".into()))
    })
    .unwrap_err();

    assert_eq!(error.code(), "INTERNAL");
    assert!(moved_focus(&error));
}

#[test]
fn the_timeout_that_ends_a_wait_after_the_raise_carries_preparatory_focus() {
    let error = marked_phase::<()>(150, true, || Err(refusal("receives_events"))).unwrap_err();

    assert_eq!(error.code(), "TIMEOUT");
    assert!(moved_focus(&error));
}

#[test]
fn the_timeout_after_attempts_with_no_focus_call_carries_no_preparatory_focus() {
    let error = marked_phase::<()>(150, false, || Err(refusal("receives_events"))).unwrap_err();

    assert_eq!(error.code(), "TIMEOUT");
    assert!(!moved_focus(&error));
}

#[test]
fn an_endpoint_that_needs_no_focus_does_not_stop_the_raise_another_endpoint_needs() {
    let _guard = HomeGuard::new();
    let snapshot_id = point_ref_snapshot();
    let ref_id = format!("@{snapshot_id}:e1");
    let adapter = CoveredTargetAdapter::new(0);
    let context = headed();
    let lease =
        crate::InteractionLease::guarded(crate::Deadline::after(TIMEOUT_MS).unwrap(), ()).unwrap();
    let args = |headed_requirement| crate::commands::point_resolve::PointResolveArgs {
        ref_id: Some(ref_id.as_str()),
        xy: None,
        missing_input_message: "",
        headed_requirement,
    };
    let mut focus = super::CommandFocus::default();

    let drop_point = focus.run(
        args(crate::HeadedRequirement::None),
        &lease,
        &adapter,
        &context,
    );
    let pickup = focus.run(
        args(crate::HeadedRequirement::FocusedWindowAndCursor),
        &lease,
        &adapter,
        &context,
    );

    assert!(!drop_point.unwrap());
    assert!(pickup.unwrap());
    assert_eq!(adapter.focuses(), 1);
}
