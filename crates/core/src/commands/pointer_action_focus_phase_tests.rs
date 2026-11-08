use crate::adapter::hit_test::HitTestResult;
use crate::adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps};
use crate::commands::pointer_action::{point_failed_check, tests::point_ref_snapshot};
use crate::commands::{drag, hover};
use crate::refs::RefEntry;
use crate::refs::refs_test_support::HomeGuard;
use crate::{AdapterError, AppError, CommandContext, DragParams, Rect};
use serde_json::json;
use std::sync::atomic::{AtomicU32, Ordering};

const TIMEOUT_MS: u64 = 1_000;
const FOREVER: u32 = u32::MAX;

struct FrontWindowAdapter {
    front: Option<Option<&'static str>>,
    clear_hit_tests: u32,
    occlusions: AtomicU32,
    hit_tests: AtomicU32,
    focuses: AtomicU32,
    leases: AtomicU32,
    focus_fails_retryably: bool,
}

impl FrontWindowAdapter {
    fn new(front: Option<Option<&'static str>>, occlusions: u32) -> Self {
        Self {
            front,
            clear_hit_tests: 0,
            occlusions: AtomicU32::new(occlusions),
            hit_tests: AtomicU32::new(0),
            focuses: AtomicU32::new(0),
            leases: AtomicU32::new(0),
            focus_fails_retryably: false,
        }
    }

    fn focuses(&self) -> u32 {
        self.focuses.load(Ordering::SeqCst)
    }

    fn leases(&self) -> u32 {
        self.leases.load(Ordering::SeqCst)
    }
}

impl ObservationOps for FrontWindowAdapter {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn get_element_bounds(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<Option<Rect>, AdapterError> {
        Ok(Some(Rect {
            x: 100.0,
            y: 200.0,
            width: 20.0,
            height: 10.0,
        }))
    }

    fn hit_test(
        &self,
        _handle: &NativeHandle,
        _point: crate::Point,
        _deadline: crate::Deadline,
    ) -> Result<HitTestResult, AdapterError> {
        let index = self.hit_tests.fetch_add(1, Ordering::SeqCst);
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
                name: Some("Transient overlay".into()),
                bounds: None,
            });
        }
        Ok(HitTestResult::ReachesTarget)
    }
    crate::adapter::saved_ref_scope!("button", Some("Target"));
}

impl ActionOps for FrontWindowAdapter {}

impl InputOps for FrontWindowAdapter {
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

impl SystemOps for FrontWindowAdapter {
    fn acquire_interaction_lease(
        &self,
        deadline: crate::Deadline,
    ) -> Result<crate::InteractionLease, AdapterError> {
        self.leases.fetch_add(1, Ordering::SeqCst);
        crate::InteractionLease::guarded(deadline, ())
    }

    fn resolve_window_strict(
        &self,
        window: &crate::WindowInfo,
        _deadline: crate::Deadline,
    ) -> Result<crate::WindowInfo, AdapterError> {
        Ok(window.clone())
    }

    fn focused_window(
        &self,
        _deadline: crate::Deadline,
    ) -> Result<Option<crate::WindowInfo>, AdapterError> {
        let Some(front) = self.front else {
            return Err(AdapterError::not_supported("focused_window"));
        };
        Ok(front.and_then(|id| {
            crate::adapter::saved_ref_window_pool()
                .into_iter()
                .find(|window| window.id == id)
        }))
    }

    fn focus_window(
        &self,
        _window: &crate::WindowInfo,
        _lease: &crate::InteractionLease,
    ) -> Result<(), AdapterError> {
        self.focuses.fetch_add(1, Ordering::SeqCst);
        if self.focus_fails_retryably {
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

fn adapter_error(error: AppError) -> AdapterError {
    match error {
        AppError::Adapter(error) => error,
        other => panic!("expected an adapter error, got {other:?}"),
    }
}

fn assert_stopped_promptly(started: std::time::Instant) {
    assert!(
        started.elapsed() < std::time::Duration::from_millis(TIMEOUT_MS / 2),
        "the command waited out its budget after raising the window"
    );
}

#[test]
fn a_target_already_in_front_keeps_retrying_a_transient_overlay() {
    let _guard = HomeGuard::new();
    let snapshot_id = point_ref_snapshot();
    let adapter = FrontWindowAdapter::new(Some(Some("w-42")), 2);

    let value = hover::execute(hover_args(&snapshot_id), &adapter, &headed()).unwrap();

    assert_eq!(adapter.focuses(), 3);
    assert_eq!(adapter.leases(), 3);
    assert_eq!(value["focused"], true);
}

#[test]
fn a_raise_over_another_window_ends_hover_with_that_attempts_refusal() {
    let _guard = HomeGuard::new();
    let snapshot_id = point_ref_snapshot();
    let adapter = FrontWindowAdapter::new(Some(Some("w-7")), FOREVER);
    let started = std::time::Instant::now();

    let error = hover::execute(hover_args(&snapshot_id), &adapter, &headed()).unwrap_err();

    assert_eq!(adapter.focuses(), 1);
    assert_eq!(error.code(), "ACTION_FAILED");
    assert!(point_failed_check(&error, "receives_events"));
    assert!(adapter_error(error).moved_focus());
    assert_eq!(adapter.leases(), 1);
    assert_stopped_promptly(started);
}

#[test]
fn a_raise_over_another_window_ends_drag_when_its_drop_point_is_covered() {
    let _guard = HomeGuard::new();
    let snapshot_id = point_ref_snapshot();
    let mut adapter = FrontWindowAdapter::new(Some(None), FOREVER);
    adapter.clear_hit_tests = 1;
    let started = std::time::Instant::now();

    let error = drag::execute(drag_args(&snapshot_id), &adapter, &headed()).unwrap_err();

    assert_eq!(adapter.focuses(), 1);
    assert_eq!(error.code(), "ACTION_FAILED");
    assert!(adapter_error(error).moved_focus());
    assert_eq!(adapter.hit_tests.load(Ordering::SeqCst), 2);
    assert_stopped_promptly(started);
}

#[test]
fn an_unreadable_front_window_retries_as_before() {
    let _guard = HomeGuard::new();
    let snapshot_id = point_ref_snapshot();
    let adapter = FrontWindowAdapter::new(None, 2);

    let value = hover::execute(hover_args(&snapshot_id), &adapter, &headed()).unwrap();

    assert_eq!(adapter.focuses(), 3);
    assert_eq!(adapter.leases(), 3);
    assert_eq!(value["focused"], true);
}

#[test]
fn a_focus_call_that_raised_over_another_window_and_failed_retryably_ends_the_command() {
    let _guard = HomeGuard::new();
    let snapshot_id = point_ref_snapshot();
    let mut adapter = FrontWindowAdapter::new(Some(Some("w-7")), 0);
    adapter.focus_fails_retryably = true;
    let started = std::time::Instant::now();

    let error = hover::execute(hover_args(&snapshot_id), &adapter, &headed()).unwrap_err();

    assert_eq!(adapter.focuses(), 1);
    assert_eq!(error.code(), "APP_UNRESPONSIVE");
    assert!(adapter_error(error).moved_focus());
    assert_stopped_promptly(started);
}
