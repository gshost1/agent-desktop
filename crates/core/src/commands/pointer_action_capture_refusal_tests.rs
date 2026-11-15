use crate::adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps};
use crate::commands::{
    drag::{self, DragArgs, DragEndpoint},
    hover::{self, HoverArgs},
};
use crate::refs::refs_store::RefStore;
use crate::refs::refs_test_support::HomeGuard;
use crate::refs::{RefEntry, RefMap};
use crate::{AdapterError, AppError, CaptureVerdict, CommandContext, Rect};
use std::sync::Mutex;

const UNPROVEN: [CaptureVerdict; 3] = [
    CaptureVerdict::Ambiguous,
    CaptureVerdict::Incomplete,
    CaptureVerdict::Unknown,
];

const TIMEOUTS: [Option<u64>; 3] = [Some(0), None, Some(2_000)];

#[derive(Default)]
struct RecordingAdapter {
    calls: Mutex<Vec<&'static str>>,
}

impl RecordingAdapter {
    fn record(&self, call: &'static str) {
        self.calls.lock().unwrap().push(call);
    }

    fn calls(&self) -> Vec<&'static str> {
        self.calls.lock().unwrap().clone()
    }
}

impl ObservationOps for RecordingAdapter {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        self.record("resolve_element");
        Ok(NativeHandle::null())
    }

    fn observe_tree(
        &self,
        root: crate::locator::live_locator::ObservationRoot<'_>,
        _request: &crate::locator::live_locator::ObservationRequest,
    ) -> Result<crate::locator::live_locator::ObservedTree, AdapterError> {
        self.record("observe_tree");
        crate::adapter::saved_ref_tree(root, "button", Some("Target"))
    }

    fn resolve_locator_anchor(
        &self,
        entry: &RefEntry,
        deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        self.resolve_element_strict(entry, deadline)
    }

    fn observe_pointer(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<crate::PointerObservation, AdapterError> {
        self.record("observe_pointer");
        Ok(crate::PointerObservation {
            bounds: Some(Rect {
                x: 100.0,
                y: 200.0,
                width: 20.0,
                height: 10.0,
            }),
            states: Vec::new(),
        })
    }

    fn hit_test(
        &self,
        _handle: &NativeHandle,
        _point: crate::Point,
        _deadline: crate::Deadline,
    ) -> Result<crate::adapter::hit_test::HitTestResult, AdapterError> {
        self.record("hit_test");
        Ok(crate::adapter::hit_test::HitTestResult::ReachesTarget)
    }

    fn list_windows(
        &self,
        _filter: &crate::adapter::WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<crate::WindowInfo>, AdapterError> {
        self.record("list_windows");
        Ok(crate::adapter::saved_ref_window_pool())
    }
}

impl ActionOps for RecordingAdapter {}

impl InputOps for RecordingAdapter {
    fn mouse_event(
        &self,
        _event: crate::MouseEvent,
        _lease: &crate::InteractionLease,
    ) -> Result<crate::OperationReport, AdapterError> {
        self.record("mouse_event");
        Ok(crate::adapter::test_report())
    }

    fn drag(
        &self,
        _params: crate::DragParams,
        _lease: &crate::InteractionLease,
    ) -> Result<crate::OperationReport, AdapterError> {
        self.record("drag");
        Ok(crate::adapter::test_report())
    }
}

impl SystemOps for RecordingAdapter {
    fn acquire_interaction_lease(
        &self,
        deadline: crate::Deadline,
    ) -> Result<crate::InteractionLease, AdapterError> {
        self.record("lease");
        crate::InteractionLease::guarded(deadline, ())
    }

    fn resolve_ref_focus_window(
        &self,
        entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<crate::WindowInfo, AdapterError> {
        self.record("resolve_focus_window");
        crate::saved_ref_focus_window(entry)
    }

    fn resolve_window_strict(
        &self,
        window: &crate::WindowInfo,
        _deadline: crate::Deadline,
    ) -> Result<crate::WindowInfo, AdapterError> {
        self.record("resolve_window");
        Ok(window.clone())
    }

    fn focus_window(
        &self,
        _window: &crate::WindowInfo,
        _lease: &crate::InteractionLease,
    ) -> Result<(), AdapterError> {
        self.record("focus_window");
        Ok(())
    }

    fn focus_app(
        &self,
        _process: crate::ProcessIdentity,
        _lease: &crate::InteractionLease,
    ) -> Result<(), AdapterError> {
        self.record("focus_app");
        Ok(())
    }
}

fn entry(name: &str, capture: CaptureVerdict) -> RefEntry {
    let mut entry = crate::adapter::minimal_ref_entry("button", Some(name));
    entry.process.pid = crate::ProcessId::new(42);
    entry.source.source_app = Some("Test".into());
    entry.source.source_window_id = Some("w-42".into());
    entry.identity.capture = capture;
    entry
}

/// Saves a snapshot whose `e1` is the unique `Target` button and whose `e2`
/// carries `capture`, and returns both qualified refs with the `e2` entry.
fn saved_refs(capture: CaptureVerdict) -> (String, String, RefEntry) {
    let unproven = entry("Twin", capture);
    let mut refmap = RefMap::new();
    refmap.allocate(entry("Target", CaptureVerdict::Unique));
    refmap.allocate(unproven.clone());
    let snapshot_id = RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap();
    (
        format!("@{snapshot_id}:e1"),
        format!("@{snapshot_id}:e2"),
        unproven,
    )
}

fn headed() -> CommandContext {
    CommandContext::default().with_headed(true)
}

fn hover_ref(ref_id: &str, timeout_ms: Option<u64>) -> HoverArgs {
    HoverArgs {
        ref_id: Some(ref_id.to_string()),
        xy: None,
        duration_ms: None,
        timeout_ms,
    }
}

fn drag_args(from: DragEndpoint, to: DragEndpoint, timeout_ms: Option<u64>) -> DragArgs {
    DragArgs {
        from,
        to,
        duration_ms: None,
        drop_delay_ms: None,
        timeout_ms,
        wait_for_scope: drag::WaitForScope::default(),
    }
}

fn endpoint_ref(ref_id: &str) -> DragEndpoint {
    DragEndpoint {
        ref_id: Some(ref_id.to_string()),
        xy: None,
    }
}

fn assert_capture_refusal(
    result: Result<serde_json::Value, AppError>,
    unproven: &RefEntry,
    adapter: &RecordingAdapter,
    case: &str,
) {
    let expected = crate::refs::resolve_errors::refused_before_reading(unproven)
        .unwrap_or_else(|| panic!("{case}: the entry must be unproven"));
    let error = match result {
        Ok(value) => panic!("{case}: an unproven ref was acted on: {value}"),
        Err(error) => error.into_adapter_error(),
    };
    assert_eq!(error.code, expected.code, "{case}");
    assert_eq!(error.message, expected.message, "{case}");
    assert_eq!(error.suggestion, expected.suggestion, "{case}");
    assert_eq!(error.details, expected.details, "{case}");
    assert_eq!(
        error.disposition,
        crate::DeliverySemantics::not_delivered(),
        "{case}"
    );
    assert!(!error.moved_focus(), "{case}");
    assert_eq!(adapter.calls(), Vec::<&str>::new(), "{case}");
}

#[test]
fn headed_hover_refuses_an_unproven_ref_before_any_adapter_call() {
    let _guard = HomeGuard::new();
    for capture in UNPROVEN {
        let (_, unproven_ref, unproven) = saved_refs(capture);
        for timeout_ms in TIMEOUTS {
            let adapter = RecordingAdapter::default();
            let result = hover::execute(hover_ref(&unproven_ref, timeout_ms), &adapter, &headed());
            let case = format!("hover {capture:?} timeout {timeout_ms:?}");
            assert_capture_refusal(result, &unproven, &adapter, &case);
        }
    }
}

#[test]
fn headed_drag_refuses_an_unproven_pickup_before_any_adapter_call() {
    let _guard = HomeGuard::new();
    for capture in UNPROVEN {
        let (_, unproven_ref, unproven) = saved_refs(capture);
        for timeout_ms in TIMEOUTS {
            let adapter = RecordingAdapter::default();
            let to = DragEndpoint {
                ref_id: None,
                xy: Some((5.0, 6.0)),
            };
            let args = drag_args(endpoint_ref(&unproven_ref), to, timeout_ms);
            let result = drag::execute(args, &adapter, &headed());
            let case = format!("drag from {capture:?} timeout {timeout_ms:?}");
            assert_capture_refusal(result, &unproven, &adapter, &case);
        }
    }
}

#[test]
fn headed_drag_refuses_an_unproven_drop_before_raising_the_pickup_window() {
    let _guard = HomeGuard::new();
    for capture in UNPROVEN {
        let (target_ref, unproven_ref, unproven) = saved_refs(capture);
        for timeout_ms in TIMEOUTS {
            let adapter = RecordingAdapter::default();
            let args = drag_args(
                endpoint_ref(&target_ref),
                endpoint_ref(&unproven_ref),
                timeout_ms,
            );
            let result = drag::execute(args, &adapter, &headed());
            let case = format!("drag to {capture:?} timeout {timeout_ms:?}");
            assert_capture_refusal(result, &unproven, &adapter, &case);
        }
    }
}

#[test]
fn a_unique_headed_hover_still_raises_its_window_once() {
    let _guard = HomeGuard::new();
    let (target_ref, _, _) = saved_refs(CaptureVerdict::Ambiguous);
    let adapter = RecordingAdapter::default();

    let value = hover::execute(hover_ref(&target_ref, None), &adapter, &headed()).unwrap();

    assert_eq!(value["focused"], true);
    let calls = adapter.calls();
    assert_eq!(
        calls.iter().filter(|call| **call == "focus_window").count(),
        1
    );
    assert!(calls.contains(&"mouse_event"));
}
