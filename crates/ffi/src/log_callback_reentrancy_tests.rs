use std::cell::{Cell, RefCell};
use std::ffi::{CStr, c_char};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use agent_desktop_core::{
    ActionOps, AdapterError, Deadline, DisplayInfo, InputOps, InteractionLease, MouseEvent,
    ObservationOps, OperationReport, ProcessLeaseGuard, SystemOps,
};

use super::ad_set_log_callback;
use crate::adapter::{AdAdapter, ad_adapter_destroy, register_adapter};
use crate::displays::list::ad_list_displays;
use crate::error::{AdResult, ad_last_error_details};
use crate::input::mouse::ad_mouse_event;
use crate::types::{AdMouseEvent, AdPoint};

const CALLBACK_HOLD: Duration = Duration::from_millis(150);

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

    fn list_displays(&self, _deadline: Deadline) -> Result<Vec<DisplayInfo>, AdapterError> {
        tracing::warn!(operation = "list_displays");
        Err(AdapterError::not_supported("list_displays"))
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

type TimedCall = (AdResult, Duration);

thread_local! {
    static ARMED_ADAPTER: Cell<*mut AdAdapter> = const { Cell::new(std::ptr::null_mut()) };
    static CALLBACK_RUNS: Cell<u32> = const { Cell::new(0) };
    static INNER_CALL: RefCell<Option<(AdResult, Duration, String)>> = const { RefCell::new(None) };
    static CONTENDER: RefCell<Option<JoinHandle<TimedCall>>> = const { RefCell::new(None) };
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

fn timed_mouse_event(adapter: *mut AdAdapter) -> TimedCall {
    let event = move_event();
    let started = Instant::now();
    let result = unsafe { ad_mouse_event(adapter, &event) };
    (result, started.elapsed())
}

unsafe extern "C" fn reenter(_level: i32, _message: *const c_char) {
    CALLBACK_RUNS.set(CALLBACK_RUNS.get().saturating_add(1));
    let adapter = ARMED_ADAPTER.replace(std::ptr::null_mut());
    if adapter.is_null() {
        return;
    }
    let (result, elapsed) = timed_mouse_event(adapter);
    INNER_CALL.set(Some((result, elapsed, last_error_kind())));
}

unsafe extern "C" fn start_contender(_level: i32, _message: *const c_char) {
    let adapter = ARMED_ADAPTER.replace(std::ptr::null_mut());
    if adapter.is_null() {
        return;
    }
    let address = adapter.expose_provenance();
    let contender = std::thread::spawn(move || {
        timed_mouse_event(std::ptr::with_exposed_provenance_mut(address))
    });
    CONTENDER.set(Some(contender));
    std::thread::sleep(CALLBACK_HOLD);
}

fn with_logging_adapter<R>(
    takes_process_lease: bool,
    callback: unsafe extern "C" fn(i32, *const c_char),
    outer_call: impl FnOnce(*mut AdAdapter) -> R,
) -> R {
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
    assert_eq!(ad_set_log_callback(Some(callback)), AdResult::Ok);
    ARMED_ADAPTER.set(adapter);
    let outer = outer_call(adapter);
    ARMED_ADAPTER.set(std::ptr::null_mut());
    assert_eq!(ad_set_log_callback(None), AdResult::Ok);
    unsafe { ad_adapter_destroy(adapter) };
    outer
}

fn inner_call() -> (AdResult, Duration, String) {
    INNER_CALL
        .take()
        .expect("the log callback must run inside the outer call")
}

#[test]
fn a_log_callback_can_call_back_into_the_library() {
    let outer = with_logging_adapter(false, reenter, |adapter| unsafe {
        let mut displays = std::ptr::null_mut();
        ad_list_displays(adapter, &mut displays)
    });
    let (inner, elapsed, kind) = inner_call();
    assert_eq!(inner, AdResult::Ok, "kind {kind:?} after {elapsed:?}");
    assert_ne!(outer, AdResult::ErrInternal);
    assert_eq!(CALLBACK_RUNS.get(), 1);
}

#[test]
fn a_mutation_from_a_log_callback_inside_a_mutation_is_refused_at_once() {
    let outer = with_logging_adapter(true, reenter, |adapter| unsafe {
        ad_mouse_event(adapter, &move_event())
    });
    let (inner, elapsed, kind) = inner_call();
    assert_eq!(outer, AdResult::Ok);
    assert_eq!(inner, AdResult::ErrPolicyDenied, "after {elapsed:?}");
    assert_eq!(kind, "reentrant_interaction_lease");
    assert!(elapsed < Duration::from_millis(100), "took {elapsed:?}");
}

#[test]
fn another_thread_waits_for_the_lease_a_logging_mutation_holds() {
    let (outer, (result, waited)) = with_logging_adapter(true, start_contender, |adapter| {
        let outer = unsafe { ad_mouse_event(adapter, &move_event()) };
        let contender = CONTENDER
            .take()
            .expect("the log callback must start the contender inside the outer call");
        (outer, contender.join().unwrap())
    });
    assert_eq!(outer, AdResult::Ok);
    assert_eq!(result, AdResult::Ok, "after {waited:?}");
    assert!(waited >= CALLBACK_HOLD / 2, "waited only {waited:?}");
}
