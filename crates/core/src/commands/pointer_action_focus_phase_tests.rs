use super::*;
use crate::commands::{drag, hover};
use crate::{CommandContext, DragParams};

struct FocusCountingAdapter {
    focuses: AtomicU32,
    leases: AtomicU32,
    focus_fails_retryably: bool,
}

impl FocusCountingAdapter {
    fn new(focus_fails_retryably: bool) -> Self {
        Self {
            focuses: AtomicU32::new(0),
            leases: AtomicU32::new(0),
            focus_fails_retryably,
        }
    }

    fn focuses(&self) -> u32 {
        self.focuses.load(Ordering::SeqCst)
    }
}

impl ObservationOps for FocusCountingAdapter {
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
        Ok(HitTestResult::InterceptedBy {
            role: Some("window".into()),
            name: Some("Another app raised over the target".into()),
            bounds: None,
        })
    }
    crate::adapter::saved_ref_scope!("button", Some("Target"));
}

impl ActionOps for FocusCountingAdapter {}

impl InputOps for FocusCountingAdapter {
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

impl SystemOps for FocusCountingAdapter {
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

const TIMEOUT_MS: u64 = 1_000;

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
            ref_id: None,
            xy: Some((400.0, 300.0)),
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

fn assert_stopped_promptly(started: std::time::Instant) {
    assert!(
        started.elapsed() < std::time::Duration::from_millis(TIMEOUT_MS / 2),
        "the command waited out its budget after raising the window"
    );
}

#[test]
fn headed_hover_ends_with_the_refusal_of_the_attempt_that_raised_the_window() {
    let _guard = HomeGuard::new();
    let snapshot_id = point_ref_snapshot();
    let adapter = FocusCountingAdapter::new(false);
    let started = std::time::Instant::now();

    let error = hover::execute(hover_args(&snapshot_id), &adapter, &headed()).unwrap_err();

    assert_eq!(error.code(), "ACTION_FAILED");
    assert!(point_failed_check(&error, "receives_events"));
    assert_eq!(adapter.focuses(), 1);
    assert_stopped_promptly(started);
    assert_eq!(adapter.leases.load(Ordering::SeqCst), 1);
}

#[test]
fn headed_drag_ends_with_the_refusal_of_the_attempt_that_raised_the_window() {
    let _guard = HomeGuard::new();
    let snapshot_id = point_ref_snapshot();
    let adapter = FocusCountingAdapter::new(false);
    let started = std::time::Instant::now();

    let error = drag::execute(drag_args(&snapshot_id), &adapter, &headed()).unwrap_err();

    assert_eq!(error.code(), "ACTION_FAILED");
    assert!(point_failed_check(&error, "receives_events"));
    assert_eq!(adapter.focuses(), 1);
    assert_stopped_promptly(started);
    assert_eq!(adapter.leases.load(Ordering::SeqCst), 1);
}

#[test]
fn a_focus_call_that_raised_and_failed_retryably_ends_the_command() {
    let _guard = HomeGuard::new();
    let snapshot_id = point_ref_snapshot();
    let adapter = FocusCountingAdapter::new(true);
    let started = std::time::Instant::now();

    let error = hover::execute(hover_args(&snapshot_id), &adapter, &headed()).unwrap_err();

    assert_eq!(error.code(), "APP_UNRESPONSIVE");
    let AppError::Adapter(error) = error else {
        panic!("expected an adapter error");
    };
    assert!(error.moved_focus());
    assert_eq!(adapter.focuses(), 1);
    assert_stopped_promptly(started);
}
