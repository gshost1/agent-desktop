use std::cell::{Cell, RefCell};
use std::ffi::{CStr, c_char};
use std::time::{Duration, Instant};

use agent_desktop_core::{
    ActionOps, AdapterError, Deadline, InputOps, InteractionLease, MouseEvent, ObservationOps,
    OperationReport, ProcessLeaseGuard, SystemOps,
};

use super::ad_set_log_callback;
use crate::adapter::{AdAdapter, ad_adapter_destroy, register_adapter};
use crate::error::{AdResult, ad_last_error_details};
use crate::input::mouse::ad_mouse_event;
use crate::types::{AdMouseEvent, AdPoint};

struct LoggingAdapter {
    takes_process_lease: bool,
}

impl ObservationOps for LoggingAdapter {}

impl ActionOps for LoggingAdapter {}

impl SystemOps for LoggingAdapter {
    fn acquire_interaction_lease(
        &self,
        deadline: Deadline,
    ) -> Result<InteractionLease, AdapterError> {
        if self.takes_process_lease {
            return InteractionLease::guarded(deadline, ProcessLeaseGuard::acquire(deadline)?);
        }
        InteractionLease::guarded(deadline, ())
    }
}

impl InputOps for LoggingAdapter {
    fn mouse_event(
        &self,
        event: MouseEvent,
        _lease: &InteractionLease,
    ) -> Result<OperationReport, AdapterError> {
        tracing::warn!(operation = "mouse_event");
        Ok(OperationReport::pointer_dispatched(&event, false))
    }
}

thread_local! {
    static ARMED_ADAPTER: Cell<*mut AdAdapter> = const { Cell::new(std::ptr::null_mut()) };
    static CALLBACK_RUNS: Cell<u32> = const { Cell::new(0) };
    static INNER_CALL: RefCell<Option<(AdResult, Duration, String)>> = const { RefCell::new(None) };
}

fn move_event() -> AdMouseEvent {
    AdMouseEvent {
        kind: 0,
        point: AdPoint { x: 1.0, y: 1.0 },
        button: 0,
        click_count: 0,
    }
}

fn last_error_kind() -> String {
    let details = ad_last_error_details();
    if details.is_null() {
        return String::new();
    }
    let text = unsafe { CStr::from_ptr(details) }.to_string_lossy();
    serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|value| value["kind"].as_str().map(str::to_owned))
        .unwrap_or_default()
}

unsafe extern "C" fn reenter(_level: i32, _message: *const c_char) {
    CALLBACK_RUNS.set(CALLBACK_RUNS.get().saturating_add(1));
    let adapter = ARMED_ADAPTER.replace(std::ptr::null_mut());
    if adapter.is_null() {
        return;
    }
    let event = move_event();
    let started = Instant::now();
    let result = unsafe { ad_mouse_event(adapter, &event) };
    let elapsed = started.elapsed();
    INNER_CALL.set(Some((result, elapsed, last_error_kind())));
}

fn run_outer_call_reentered_by_callback(
    takes_process_lease: bool,
) -> (AdResult, AdResult, Duration, String) {
    let _serial = super::tests::log_test_serialization();
    let adapter = register_adapter(AdAdapter {
        inner: Box::new(LoggingAdapter {
            takes_process_lease,
        }),
        session_id: None,
        _session_lease: None,
    })
    .unwrap();
    CALLBACK_RUNS.set(0);
    assert_eq!(ad_set_log_callback(Some(reenter)), AdResult::Ok);
    ARMED_ADAPTER.set(adapter);
    let outer = unsafe { ad_mouse_event(adapter, &move_event()) };
    ARMED_ADAPTER.set(std::ptr::null_mut());
    assert_eq!(ad_set_log_callback(None), AdResult::Ok);
    unsafe { ad_adapter_destroy(adapter) };
    let (inner, elapsed, kind) = INNER_CALL
        .take()
        .expect("the log callback must run inside the outer call");
    (outer, inner, elapsed, kind)
}

#[test]
fn a_log_callback_can_call_back_into_the_library() {
    let (outer, inner, elapsed, kind) = run_outer_call_reentered_by_callback(false);
    assert_eq!(inner, AdResult::Ok, "kind {kind:?} after {elapsed:?}");
    assert_eq!(outer, AdResult::Ok);
    assert_eq!(CALLBACK_RUNS.get(), 1);
}
