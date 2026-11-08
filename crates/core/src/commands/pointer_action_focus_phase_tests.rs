use crate::adapter::hit_test::HitTestResult;
use crate::adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps};
use crate::commands::{drag, hover};
use crate::refs::refs_store::RefStore;
use crate::refs::refs_test_support::HomeGuard;
use crate::refs::{RefEntry, RefMap};
use crate::{AdapterError, CommandContext, DragParams, ErrorCode, Rect};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

struct FocusCountingAdapter {
    focuses: AtomicU32,
    focus_failures_left: AtomicU32,
    occlusions_left: AtomicU32,
    hidden_until_focused: bool,
    raised: AtomicBool,
    scrolls: AtomicU32,
    leases: AtomicU32,
}

impl FocusCountingAdapter {
    fn occluded_for(attempts: u32) -> Self {
        Self {
            focuses: AtomicU32::new(0),
            focus_failures_left: AtomicU32::new(0),
            occlusions_left: AtomicU32::new(attempts),
            hidden_until_focused: false,
            raised: AtomicBool::new(false),
            scrolls: AtomicU32::new(0),
            leases: AtomicU32::new(0),
        }
    }

    fn hidden_until_focused(mut self) -> Self {
        self.hidden_until_focused = true;
        self
    }

    fn failing_focus(self, failures: u32) -> Self {
        self.focus_failures_left.store(failures, Ordering::SeqCst);
        self
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

    fn observe_pointer(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<crate::PointerObservation, AdapterError> {
        let states = if self.hidden_until_focused && !self.raised.load(Ordering::SeqCst) {
            vec![crate::tree::state::OFFSCREEN.to_string()]
        } else {
            Vec::new()
        };
        Ok(crate::PointerObservation {
            bounds: Some(Rect {
                x: 100.0,
                y: 200.0,
                width: 20.0,
                height: 10.0,
            }),
            states,
        })
    }

    fn hit_test(
        &self,
        _handle: &NativeHandle,
        _point: crate::Point,
        _deadline: crate::Deadline,
    ) -> Result<HitTestResult, AdapterError> {
        let occluded = self
            .occlusions_left
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                left.checked_sub(1)
            })
            .is_ok();
        if occluded {
            return Ok(HitTestResult::InterceptedBy {
                role: Some("AXSheet".into()),
                name: Some("Transient overlay".into()),
                bounds: None,
            });
        }
        Ok(HitTestResult::ReachesTarget)
    }
    crate::adapter::saved_ref_scope!("button", Some("Target"));
}

impl ActionOps for FocusCountingAdapter {
    fn scroll_into_view(
        &self,
        _handle: &NativeHandle,
        _lease: &crate::InteractionLease,
    ) -> Result<(), AdapterError> {
        self.scrolls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

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
        let failed = self
            .focus_failures_left
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                left.checked_sub(1)
            })
            .is_ok();
        if failed {
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "The window manager refused to raise the window",
            ));
        }
        self.raised.store(true, Ordering::SeqCst);
        Ok(())
    }
}

fn snapshot() -> String {
    let store = RefStore::new().unwrap();
    let mut refmap = RefMap::new();
    for pid in [42, 99] {
        refmap.allocate(RefEntry {
            process: crate::RefProcess {
                pid: crate::ProcessId::new(pid),
                process_instance: Some("test-instance".into()),
            },
            identity: crate::RefEntryIdentity {
                retained_object: None,
                role: "button".into(),
                name: Some("Target".into()),
                value: None,
                description: None,
                native_id: None,
                unique_at_capture: true,
            },
            geometry: crate::RefGeometry {
                bounds_space: None,
                bounds: None,
                bounds_hash: None,
            },
            capabilities: crate::RefCapabilities {
                states: vec![],
                available_actions: vec![crate::actions::capability::CLICK.into()],
            },
            source: crate::RefSource {
                source_app: Some(format!("App {pid}")),
                source_window_id: Some(format!("w-{pid}")),
                source_window_title: Some(format!("Window {pid}")),
                source_window_bounds_hash: None,
                source_surface: crate::adapter::SnapshotSurface::Window,
                renderer_accessibility_forced: false,
            },
            scope: crate::RefScope {
                root_ref: None,
                path_is_absolute: false,
                path: smallvec::SmallVec::new(),
            },
        });
    }
    store.save_new_snapshot(&refmap).unwrap()
}

fn hover_args() -> hover::HoverArgs {
    hover::HoverArgs {
        ref_id: Some(format!("@{}:e1", snapshot())),
        xy: None,
        duration_ms: None,
        timeout_ms: Some(5_000),
    }
}

fn drag_args() -> drag::DragArgs {
    let snapshot_id = snapshot();
    drag::DragArgs {
        from: drag::DragEndpoint {
            ref_id: Some(format!("@{snapshot_id}:e1")),
            xy: None,
        },
        to: drag::DragEndpoint {
            ref_id: Some(format!("@{snapshot_id}:e2")),
            xy: None,
        },
        duration_ms: None,
        drop_delay_ms: None,
        timeout_ms: Some(5_000),
        wait_for_scope: drag::WaitForScope::default(),
    }
}

fn headed() -> CommandContext {
    CommandContext::default().with_headed(true)
}

#[test]
fn headed_hover_raises_the_window_once_across_retried_attempts() {
    let _guard = HomeGuard::new();
    let adapter = FocusCountingAdapter::occluded_for(2);

    let value = hover::execute(hover_args(), &adapter, &headed()).unwrap();

    assert_eq!(adapter.leases.load(Ordering::SeqCst), 3);
    assert_eq!(adapter.focuses(), 1);
    assert_eq!(value["focused"], true);
}

#[test]
fn headed_drag_raises_the_source_window_once_across_retried_attempts() {
    let _guard = HomeGuard::new();
    let adapter = FocusCountingAdapter::occluded_for(2);

    let value = drag::execute(drag_args(), &adapter, &headed()).unwrap();

    assert_eq!(adapter.leases.load(Ordering::SeqCst), 3);
    assert_eq!(adapter.focuses(), 1);
    assert_eq!(value["focused"], true);
}

#[test]
fn a_focus_run_by_the_wait_scroll_is_reused_by_the_leased_attempts() {
    let _guard = HomeGuard::new();
    let adapter = FocusCountingAdapter::occluded_for(1).hidden_until_focused();

    let value = hover::execute(hover_args(), &adapter, &headed()).unwrap();

    assert_eq!(adapter.scrolls.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.focuses(), 1);
    assert_eq!(value["focused"], true);
}

#[test]
fn a_failed_focus_stays_the_command_failure() {
    let _guard = HomeGuard::new();
    let adapter = FocusCountingAdapter::occluded_for(0).failing_focus(1);

    let error = hover::execute(hover_args(), &adapter, &headed()).unwrap_err();

    assert_eq!(error.code(), "ACTION_FAILED");
    assert_eq!(adapter.focuses(), 1);
}
