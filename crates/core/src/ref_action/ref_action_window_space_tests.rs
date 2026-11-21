use crate::adapter::hit_test::HitTestResult;
use crate::adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps};
use crate::ref_action::*;
use crate::{
    ErrorCode, actions::action::Action, refs::RefEntry, tree::element_state::ElementState,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

/// A list row with no semantic action, read live on screen while its refmap
/// entry carries the same rectangle measured inside its window.
struct RowWithoutAction;

fn live_on_screen() -> crate::Rect {
    crate::Rect {
        x: 471.0,
        y: 499.0,
        width: 230.0,
        height: 43.0,
    }
}

fn in_window() -> crate::Rect {
    crate::Rect {
        x: 1.0,
        y: 279.0,
        width: 230.0,
        height: 43.0,
    }
}

impl ObservationOps for RowWithoutAction {
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
        Ok(crate::LiveElement {
            identity: crate::adapter::live_identity("Displays"),
            state: ElementState {
                role: "listitem".into(),
                states: Vec::new(),
                value: None,
                enabled: Some(true),
                hidden: Some(false),
                offscreen: Some(false),
            },
            states_complete: true,
            bounds: Some(live_on_screen()),
            available_actions: Vec::new(),
        })
    }

    fn get_element_bounds(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<Option<crate::Rect>, AdapterError> {
        Ok(Some(live_on_screen()))
    }
    crate::adapter::saved_ref_scope!("listitem", Some("Displays"));
}

impl ActionOps for RowWithoutAction {}
impl InputOps for RowWithoutAction {}
impl SystemOps for RowWithoutAction {
    crate::adapter::guarded_interaction_lease!();
}

fn entry(space: Option<crate::BoundsSpace>, bounds: crate::Rect) -> RefEntry {
    let mut entry = crate::adapter::minimal_ref_entry("listitem", Some("Displays"));
    entry.geometry = crate::RefGeometry {
        bounds_space: space,
        bounds: Some(bounds),
        bounds_hash: bounds.bounds_hash(),
        screen_bounds_hash: None,
    };
    entry
}

fn stable_check(error: &AdapterError) -> Option<String> {
    error
        .details
        .as_ref()?
        .get("checks")?
        .as_array()?
        .iter()
        .find(|check| check["check"] == "stable")
        .map(|check| check["status"].as_str().unwrap_or_default().to_owned())
}

fn click(entry: &RefEntry) -> AdapterError {
    let request = ActionRequest::headless(Action::Click).with_timeout_ms(Some(400));
    execute_entry(&RowWithoutAction, entry, request).unwrap_err()
}

#[test]
fn a_window_space_ref_is_refused_for_its_real_reason_alone() {
    let error = click(&entry(Some(crate::BoundsSpace::Window), in_window()));

    assert_ne!(error.code, ErrorCode::Timeout, "{error:?}");
    assert!(!error.message.contains("stable"), "{}", error.message);
    assert_ne!(stable_check(&error).as_deref(), Some("fail"), "{error:?}");
    assert!(
        error.message.contains("supported_action"),
        "{}",
        error.message
    );
}

#[test]
fn a_screen_space_ref_that_moved_still_fails_the_stable_check() {
    let error = click(&entry(None, in_window()));

    assert!(error.message.contains("stable"), "{}", error.message);
    assert_eq!(stable_check(&error).as_deref(), Some("fail"), "{error:?}");
}

struct LeaseFlag(Arc<AtomicBool>);

impl Drop for LeaseFlag {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

struct CountingRow {
    live_rect: crate::Rect,
    in_lease: Arc<AtomicBool>,
    live_reads_under_lease: AtomicUsize,
    bounds_reads_under_lease: AtomicUsize,
    bounds_reads_total: AtomicUsize,
    dispatches: AtomicUsize,
}

impl CountingRow {
    fn new(live_rect: crate::Rect) -> Self {
        Self {
            live_rect,
            in_lease: Arc::new(AtomicBool::new(false)),
            live_reads_under_lease: AtomicUsize::new(0),
            bounds_reads_under_lease: AtomicUsize::new(0),
            bounds_reads_total: AtomicUsize::new(0),
            dispatches: AtomicUsize::new(0),
        }
    }
}

impl ObservationOps for CountingRow {
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
        if self.in_lease.load(Ordering::SeqCst) {
            self.live_reads_under_lease.fetch_add(1, Ordering::SeqCst);
        }
        Ok(crate::LiveElement {
            identity: crate::adapter::live_identity("Displays"),
            state: ElementState {
                role: "listitem".into(),
                states: Vec::new(),
                value: None,
                enabled: Some(true),
                hidden: Some(false),
                offscreen: Some(false),
            },
            states_complete: true,
            bounds: Some(self.live_rect),
            available_actions: Vec::new(),
        })
    }

    fn get_element_bounds(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<Option<crate::Rect>, AdapterError> {
        self.bounds_reads_total.fetch_add(1, Ordering::SeqCst);
        if self.in_lease.load(Ordering::SeqCst) {
            self.bounds_reads_under_lease.fetch_add(1, Ordering::SeqCst);
        }
        Ok(Some(self.live_rect))
    }

    fn hit_test(
        &self,
        _handle: &NativeHandle,
        _point: crate::Point,
        _deadline: crate::Deadline,
    ) -> Result<HitTestResult, AdapterError> {
        Ok(HitTestResult::ReachesTarget)
    }

    crate::adapter::saved_ref_scope!("listitem", Some("Displays"));
}

impl ActionOps for CountingRow {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
        _lease: &crate::InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        self.dispatches.fetch_add(1, Ordering::SeqCst);
        Ok(ActionResult::delivered_unverified("click"))
    }
}

impl InputOps for CountingRow {}

impl SystemOps for CountingRow {
    fn acquire_interaction_lease(
        &self,
        deadline: crate::Deadline,
    ) -> Result<crate::InteractionLease, AdapterError> {
        self.in_lease.store(true, Ordering::SeqCst);
        crate::InteractionLease::guarded(deadline, LeaseFlag(Arc::clone(&self.in_lease)))
    }

    crate::adapter::exact_window_focus!();
}

fn window_entry_with_screen(window: crate::Rect, screen: Option<crate::Rect>) -> RefEntry {
    let mut entry = entry(Some(crate::BoundsSpace::Window), window);
    entry.geometry.screen_bounds_hash = screen.and_then(|rect| rect.bounds_hash());
    entry.source.source_app = Some("Test".into());
    entry.source.source_window_id = Some("w-1".into());
    entry.source.source_window_title = Some("Window 1".into());
    entry
}

#[test]
fn unmoved_non_origin_window_space_ref_skips_sampler_under_lease() {
    let adapter = CountingRow::new(live_on_screen());
    let target = window_entry_with_screen(in_window(), Some(live_on_screen()));
    let request = ActionRequest::headed(Action::Click).with_timeout_ms(Some(400));

    let result = execute_entry(&adapter, &target, request);

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(adapter.dispatches.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.live_reads_under_lease.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.bounds_reads_under_lease.load(Ordering::SeqCst), 0);
}

#[test]
fn window_moved_to_origin_still_runs_stability_sampler() {
    let snapshot_screen = crate::Rect {
        x: 101.0,
        y: 379.0,
        width: 230.0,
        height: 43.0,
    };
    let adapter = CountingRow::new(in_window());
    let target = window_entry_with_screen(in_window(), Some(snapshot_screen));
    let request = ActionRequest::headed(Action::Click).with_timeout_ms(Some(400));

    let result = execute_entry(&adapter, &target, request);

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(adapter.dispatches.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.live_reads_under_lease.load(Ordering::SeqCst), 2);
    assert!(
        adapter.bounds_reads_under_lease.load(Ordering::SeqCst) >= 3,
        "sampler must sample live screen bounds when window moved from (100,100) to (0,0)"
    );
}

#[test]
fn window_space_ref_without_screen_hash_runs_stability_sampler() {
    let adapter = CountingRow::new(live_on_screen());
    let target = window_entry_with_screen(in_window(), None);
    let request = ActionRequest::headed(Action::Click).with_timeout_ms(Some(400));

    let result = execute_entry(&adapter, &target, request);

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(adapter.dispatches.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.live_reads_under_lease.load(Ordering::SeqCst), 2);
    assert!(adapter.bounds_reads_under_lease.load(Ordering::SeqCst) >= 3);
}

#[test]
fn headless_window_space_ref_with_screen_hash_refuses_without_live_priming() {
    let adapter = CountingRow::new(live_on_screen());
    let target = window_entry_with_screen(in_window(), Some(live_on_screen()));
    let request = ActionRequest::headless(Action::Click).with_timeout_ms(Some(400));

    let error = execute_entry(&adapter, &target, request).unwrap_err();

    assert_ne!(error.code, ErrorCode::Timeout, "{error:?}");
    assert!(!error.message.contains("stable"), "{}", error.message);
    assert!(
        error.message.contains("supported_action"),
        "{}",
        error.message
    );
    assert_eq!(adapter.bounds_reads_total.load(Ordering::SeqCst), 0);
}
