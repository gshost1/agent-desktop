use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::system::process_state::ProcessState;
use crate::wait::deadline::{VirtualClock, sleep};
use crate::{AdapterError, DeliverySemantics, ErrorCode};

const EXIT_TAKES: Duration = Duration::from_millis(1_000);

#[derive(Clone, Copy)]
enum Lease {
    GrantedJustBeforeDeadline,
    TimesOut,
}

#[derive(Clone, Copy)]
enum Close {
    ExitsAfterRequest,
    ExitsBeforeRequest,
}

struct PhasedAdapter {
    lease: Lease,
    close: Close,
    exited: AtomicBool,
    close_calls: AtomicUsize,
    lease_budget: Mutex<Option<Duration>>,
}

impl PhasedAdapter {
    fn new(lease: Lease, close: Close) -> Self {
        Self {
            lease,
            close,
            exited: AtomicBool::new(false),
            close_calls: AtomicUsize::new(0),
            lease_budget: Mutex::new(None),
        }
    }
}

impl ObservationOps for PhasedAdapter {
    fn list_apps(&self, _deadline: crate::Deadline) -> Result<Vec<crate::AppInfo>, AdapterError> {
        Ok(vec![crate::AppInfo {
            name: "Editor".into(),
            pid: crate::ProcessId::new(91),
            bundle_id: None,
            process_instance: Some("editor-instance".into()),
            presentation: None,
        }])
    }
}

impl ActionOps for PhasedAdapter {}

impl InputOps for PhasedAdapter {}

impl SystemOps for PhasedAdapter {
    fn acquire_interaction_lease(
        &self,
        deadline: crate::Deadline,
    ) -> Result<crate::InteractionLease, AdapterError> {
        if let Ok(mut budget) = self.lease_budget.lock() {
            *budget = Some(deadline.remaining());
        }
        match self.lease {
            Lease::GrantedJustBeforeDeadline => {
                sleep(
                    deadline
                        .remaining()
                        .saturating_sub(Duration::from_millis(1)),
                );
                crate::InteractionLease::guarded(deadline, ())
            }
            Lease::TimesOut => {
                sleep(deadline.remaining());
                Err(deadline.timeout_error())
            }
        }
    }

    fn close_app(
        &self,
        app: &crate::AppInfo,
        _force: bool,
        lease: &crate::InteractionLease,
    ) -> Result<crate::OperationReport, AdapterError> {
        self.close_calls.fetch_add(1, Ordering::SeqCst);
        match self.close {
            Close::ExitsBeforeRequest => {
                self.exited.store(true, Ordering::SeqCst);
                Err(AdapterError::new(
                    ErrorCode::AppUnresponsive,
                    "Target process instance is no longer running",
                ))
            }
            Close::ExitsAfterRequest => {
                let remaining = lease.deadline().remaining();
                if remaining < EXIT_TAKES {
                    sleep(remaining);
                    return crate::OperationReport::from_close_dispatch(
                        app,
                        Err(lease
                            .deadline()
                            .timeout_error()
                            .with_disposition(DeliverySemantics::delivered_unverified())),
                    );
                }
                sleep(EXIT_TAKES);
                self.exited.store(true, Ordering::SeqCst);
                crate::OperationReport::from_close_dispatch(app, Ok(true))
            }
        }
    }

    fn process_state(
        &self,
        _process: crate::ProcessIdentity,
        _deadline: crate::Deadline,
    ) -> Result<ProcessState, AdapterError> {
        if self.exited.load(Ordering::SeqCst) {
            Ok(ProcessState::Exited { code: None })
        } else {
            Ok(ProcessState::Running)
        }
    }
}

fn close_args() -> CloseAppArgs {
    close_args_within(None)
}

fn close_args_within(timeout_ms: Option<u64>) -> CloseAppArgs {
    CloseAppArgs {
        app: "Editor".into(),
        force: false,
        timeout_ms,
    }
}

#[test]
fn contended_lease_still_leaves_the_exit_wait_its_reserved_budget() {
    let _clock = VirtualClock::freeze();
    let adapter = PhasedAdapter::new(Lease::GrantedJustBeforeDeadline, Close::ExitsAfterRequest);

    let value = execute(close_args(), &adapter).unwrap();

    assert_eq!(value["disposition"]["delivery"], "delivered_verified");
    assert_eq!(value["details"]["evidence"], "process_instance_exited");
}

#[test]
fn app_that_exits_during_the_close_is_already_satisfied_not_unresponsive() {
    let _clock = VirtualClock::freeze();
    let adapter = PhasedAdapter::new(Lease::GrantedJustBeforeDeadline, Close::ExitsBeforeRequest);

    let value = execute(close_args(), &adapter).unwrap();

    assert_eq!(value["disposition"]["delivery"], "not_delivered");
    assert_eq!(value["disposition"]["retry"], "safe");
    assert_eq!(value["details"]["evidence"], "process_instance_not_running");
}

#[test]
fn lease_timeout_before_the_close_is_sent_is_timeout_not_delivered() {
    let _clock = VirtualClock::freeze();
    let adapter = PhasedAdapter::new(Lease::TimesOut, Close::ExitsAfterRequest);

    let err = execute(close_args(), &adapter).unwrap_err();

    assert_eq!(err.code(), "TIMEOUT");
    let AppError::Adapter(error) = err else {
        panic!("lease timeout must stay an adapter error");
    };
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(adapter.close_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn timeout_option_sets_the_total_the_phases_split() {
    let _clock = VirtualClock::freeze();
    let adapter = PhasedAdapter::new(Lease::GrantedJustBeforeDeadline, Close::ExitsAfterRequest);

    execute(close_args_within(Some(10_000)), &adapter).unwrap();

    let granted = adapter.lease_budget.lock().unwrap().unwrap();
    assert_eq!(granted, Duration::from_millis(5_000));
}

#[test]
fn process_still_running_after_a_failed_close_keeps_the_failure() {
    let _clock = VirtualClock::freeze();
    let adapter = PhasedAdapter::new(Lease::GrantedJustBeforeDeadline, Close::ExitsAfterRequest);

    let err = execute(close_args_within(Some(1_500)), &adapter).unwrap_err();

    assert_eq!(err.code(), "TIMEOUT");
    let AppError::Adapter(error) = err else {
        panic!("exit timeout must stay an adapter error");
    };
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}
