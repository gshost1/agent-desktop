use std::cell::{Cell, RefCell};
use std::ffi::{CStr, CString, c_char};
use std::sync::MutexGuard;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use agent_desktop_core::{
    ActionOps, AdapterError, Deadline, DisplayInfo, InputOps, InteractionLease, MouseEvent,
    ObservationOps, OperationReport, ProcessLeaseGuard, SystemOps,
};

use super::ad_set_log_callback;
use crate::adapter::{AdAdapter, ad_adapter_destroy, register_adapter};
use crate::apps::close::ad_close_app;
use crate::apps::launch::ad_launch_app_exact;
use crate::commands::execute_by_ref_timeout::ad_execute_by_ref_timeout;
use crate::displays::list::ad_list_displays;
use crate::error::{AdResult, ad_last_error_details};
use crate::input::clipboard::ad_free_string;
use crate::input::mouse::ad_mouse_event;
use crate::types::{AdAction, AdExactWindowInfo, AdMouseEvent, AdPoint};

const CALLBACK_HOLD: Duration = Duration::from_millis(150);

struct LoggingAdapter {
    takes_process_lease: bool,
    logs_while_acquiring: bool,
}

impl ObservationOps for LoggingAdapter {}

impl ActionOps for LoggingAdapter {}

impl SystemOps for LoggingAdapter {
    fn acquire_interaction_lease(
        &self,
        deadline: Deadline,
    ) -> Result<InteractionLease, AdapterError> {
        if self.logs_while_acquiring {
            tracing::warn!(operation = "acquire_interaction_lease");
        }
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

type InnerCall = (AdResult, Duration, String);
type ContenderOutcome = (AdResult, Duration, Option<InnerCall>);
type Action = fn(*mut AdAdapter);

thread_local! {
    static ARMED: Cell<Option<(Action, *mut AdAdapter)>> = const { Cell::new(None) };
    static CALLBACK_RUNS: Cell<u32> = const { Cell::new(0) };
    static INNER_CALL: RefCell<Option<InnerCall>> = const { RefCell::new(None) };
    static SECOND_ADAPTER: Cell<usize> = const { Cell::new(0) };
    static CONTENDER: RefCell<Option<JoinHandle<ContenderOutcome>>> = const { RefCell::new(None) };
}

unsafe extern "C" fn run_armed_action(_level: i32, _message: *const c_char) {
    CALLBACK_RUNS.set(CALLBACK_RUNS.get().saturating_add(1));
    if let Some((action, adapter)) = ARMED.take() {
        action(adapter);
    }
}

struct LoggingSession {
    adapters: Vec<*mut AdAdapter>,
    _serial: MutexGuard<'static, ()>,
}

impl LoggingSession {
    fn start() -> Option<Self> {
        #[cfg(unix)]
        if std::env::var_os(agent_desktop_core::INTERACTION_LEASE_FD_ENV).is_some() {
            eprintln!("skipped: an inherited lease fd routes FFI leases past the test adapter");
            return None;
        }
        let session = Self {
            adapters: Vec::new(),
            _serial: super::tests::log_test_serialization(),
        };
        CALLBACK_RUNS.set(0);
        assert_eq!(ad_set_log_callback(Some(run_armed_action)), AdResult::Ok);
        Some(session)
    }

    fn adapter(&mut self, takes_process_lease: bool, logs_while_acquiring: bool) -> *mut AdAdapter {
        let adapter = register_adapter(AdAdapter {
            inner: Box::new(LoggingAdapter {
                takes_process_lease,
                logs_while_acquiring,
            }),
            session_id: None,
            _session_lease: None,
        })
        .unwrap();
        self.adapters.push(adapter);
        adapter
    }
}

impl Drop for LoggingSession {
    fn drop(&mut self) {
        let _ = ad_set_log_callback(None);
        ARMED.set(None);
        for adapter in self.adapters.drain(..) {
            unsafe { ad_adapter_destroy(adapter) };
        }
    }
}

fn move_event() -> AdMouseEvent {
    AdMouseEvent {
        kind: 0,
        point: AdPoint { x: 1.0, y: 1.0 },
        button: 0,
        click_count: 0,
    }
}

fn mouse_event(adapter: *mut AdAdapter) -> AdResult {
    unsafe { ad_mouse_event(adapter, &move_event()) }
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

fn record(call: impl FnOnce() -> AdResult) {
    let started = Instant::now();
    let result = call();
    let elapsed = started.elapsed();
    INNER_CALL.set(Some((result, elapsed, last_error_kind())));
}

fn reenter_mouse_event(adapter: *mut AdAdapter) {
    record(|| mouse_event(adapter));
}

fn reenter_launch_app_exact(adapter: *mut AdAdapter) {
    let id = CString::new("org.example.app").unwrap();
    let mut out: AdExactWindowInfo = unsafe { std::mem::zeroed() };
    record(|| unsafe { ad_launch_app_exact(adapter, id.as_ptr(), 1_000, &mut out) });
}

fn reenter_execute_by_ref_timeout(adapter: *mut AdAdapter) {
    let ref_id = CString::new("@s8f3k2p9:e1").unwrap();
    let action: AdAction = unsafe { std::mem::zeroed() };
    let mut out: *mut c_char = std::ptr::null_mut();
    record(|| unsafe {
        ad_execute_by_ref_timeout(adapter, ref_id.as_ptr(), &action, 0, -1, &mut out)
    });
    unsafe { ad_free_string(out) };
}

fn reenter_close_app(adapter: *mut AdAdapter) {
    let id = CString::new("org.example.app").unwrap();
    record(|| unsafe { ad_close_app(adapter, id.as_ptr(), false) });
}

fn spawn_contender(address: usize, inner: Option<Action>) -> JoinHandle<ContenderOutcome> {
    std::thread::spawn(move || {
        let adapter = std::ptr::with_exposed_provenance_mut(address);
        ARMED.set(inner.map(|action| (action, adapter)));
        let started = Instant::now();
        let result = mouse_event(adapter);
        (result, started.elapsed(), INNER_CALL.take())
    })
}

fn start_contender(adapter: *mut AdAdapter) {
    CONTENDER.set(Some(spawn_contender(adapter.expose_provenance(), None)));
    std::thread::sleep(CALLBACK_HOLD);
}

fn start_waiting_reentrant_contender(_adapter: *mut AdAdapter) {
    let contender = spawn_contender(SECOND_ADAPTER.get(), Some(reenter_mouse_event));
    CONTENDER.set(Some(contender));
    std::thread::sleep(CALLBACK_HOLD);
}

fn joined_contender() -> ContenderOutcome {
    CONTENDER
        .take()
        .expect("the log callback must start the contender inside the outer call")
        .join()
        .unwrap()
}

fn inner_call() -> InnerCall {
    INNER_CALL
        .take()
        .expect("the log callback must run inside the outer call")
}

#[test]
fn a_log_callback_can_call_back_into_the_library() {
    let Some(mut session) = LoggingSession::start() else {
        return;
    };
    let adapter = session.adapter(false, false);
    ARMED.set(Some((reenter_mouse_event, adapter)));
    let mut displays = std::ptr::null_mut();
    let outer = unsafe { ad_list_displays(adapter, &mut displays) };
    let (inner, elapsed, kind) = inner_call();
    assert_eq!(inner, AdResult::Ok, "kind {kind:?} after {elapsed:?}");
    assert_ne!(outer, AdResult::ErrInternal);
    assert_eq!(CALLBACK_RUNS.get(), 1);
}

#[test]
fn every_lease_taking_mutation_from_a_log_callback_inside_a_mutation_is_refused_at_once() {
    let entry_points: [(&str, Action); 4] = [
        ("ad_mouse_event", reenter_mouse_event),
        ("ad_launch_app_exact", reenter_launch_app_exact),
        ("ad_execute_by_ref_timeout", reenter_execute_by_ref_timeout),
        ("ad_close_app", reenter_close_app),
    ];
    for (name, action) in entry_points {
        let Some(mut session) = LoggingSession::start() else {
            return;
        };
        let adapter = session.adapter(true, false);
        ARMED.set(Some((action, adapter)));
        let outer = mouse_event(adapter);
        let (inner, elapsed, kind) = inner_call();
        assert_eq!(outer, AdResult::Ok, "{name}");
        assert_eq!(inner, AdResult::ErrPolicyDenied, "{name} after {elapsed:?}");
        assert_eq!(kind, "reentrant_interaction_lease", "{name}");
        assert!(
            elapsed < Duration::from_millis(100),
            "{name} took {elapsed:?}"
        );
    }
}

#[test]
fn another_thread_waits_for_the_lease_a_logging_mutation_holds() {
    let Some(mut session) = LoggingSession::start() else {
        return;
    };
    let adapter = session.adapter(true, false);
    ARMED.set(Some((start_contender, adapter)));
    let outer = mouse_event(adapter);
    let (result, waited, _) = joined_contender();
    assert_eq!(outer, AdResult::Ok);
    assert_eq!(result, AdResult::Ok, "after {waited:?}");
    assert!(waited >= CALLBACK_HOLD / 2, "waited only {waited:?}");
}

#[test]
fn a_thread_still_waiting_for_the_lease_is_not_refused_from_its_callback() {
    let Some(mut session) = LoggingSession::start() else {
        return;
    };
    let holder = session.adapter(true, false);
    let waiter = session.adapter(true, true);
    SECOND_ADAPTER.set(waiter.expose_provenance());
    ARMED.set(Some((start_waiting_reentrant_contender, holder)));
    let outer = mouse_event(holder);
    let (result, waited, inner) = joined_contender();
    let (inner, inner_elapsed, kind) = inner.expect("the waiting thread's callback must run");
    assert_eq!(outer, AdResult::Ok);
    assert_eq!(inner, AdResult::Ok, "kind {kind:?} after {inner_elapsed:?}");
    assert_eq!(result, AdResult::Ok, "after {waited:?}");
}
