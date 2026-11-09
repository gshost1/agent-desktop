use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::system::process_state::ProcessState;
use crate::wait::deadline::{VirtualClock, now, sleep};
use crate::{AdapterError, DeliverySemantics, ErrorCode};

const EXIT_TAKES: Duration = Duration::from_millis(1_000);

#[derive(Clone, Copy)]
enum Lease {
    GrantedJustBeforeDeadline,
    TimesOut,
}

enum Close {
    ExitsAfterRequest,
    FailsThenExits {
        code: ErrorCode,
        disposition: DeliverySemantics,
        exit_after: Duration,
    },
}

struct PhasedAdapter {
    lease: Lease,
    close: Close,
    slow_inventory: bool,
    exit_at: Mutex<Option<Instant>>,
    close_calls: AtomicUsize,
    lease_budget: Mutex<Option<Duration>>,
}

impl PhasedAdapter {
    fn new(lease: Lease, close: Close) -> Self {
        Self {
            lease,
            close,
            slow_inventory: false,
            exit_at: Mutex::new(None),
            close_calls: AtomicUsize::new(0),
            lease_budget: Mutex::new(None),
        }
    }

    fn exit_in(&self, delay: Duration) {
        *self.exit_at.lock().unwrap() = Some(now() + delay);
    }
}

impl ObservationOps for PhasedAdapter {
    fn list_apps(&self, deadline: crate::Deadline) -> Result<Vec<crate::AppInfo>, AdapterError> {
        if self.slow_inventory {
            sleep(deadline.remaining());
            return Err(deadline.timeout_error());
        }
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
        *self.lease_budget.lock().unwrap() = Some(deadline.remaining());
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
        match &self.close {
            Close::FailsThenExits {
                code,
                disposition,
                exit_after,
            } => {
                self.exit_in(*exit_after);
                Err(AdapterError::new(code.clone(), "close failed").with_disposition(*disposition))
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
                self.exit_in(Duration::ZERO);
                crate::OperationReport::from_close_dispatch(app, Ok(true))
            }
        }
    }

    fn process_state(
        &self,
        _process: crate::ProcessIdentity,
        _deadline: crate::Deadline,
    ) -> Result<ProcessState, AdapterError> {
        let exited = self.exit_at.lock().unwrap().is_some_and(|at| now() >= at);
        Ok(if exited {
            ProcessState::Exited { code: None }
        } else {
            ProcessState::Running
        })
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

fn fails_then_exits(code: ErrorCode, disposition: DeliverySemantics) -> PhasedAdapter {
    PhasedAdapter::new(
        Lease::GrantedJustBeforeDeadline,
        Close::FailsThenExits {
            code,
            disposition,
            exit_after: Duration::ZERO,
        },
    )
}

fn adapter_error(err: AppError) -> AdapterError {
    match err {
        AppError::Adapter(error) => error,
        other => panic!("expected an adapter error, got {other:?}"),
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
fn failed_close_of_unknown_or_uncertain_delivery_then_gone_is_delivered_unverified() {
    for disposition in [DeliverySemantics::unknown(), DeliverySemantics::uncertain()] {
        let _clock = VirtualClock::freeze();
        let adapter = fails_then_exits(ErrorCode::ActionFailed, disposition);

        let value = execute(close_args(), &adapter).unwrap();

        assert_eq!(value["disposition"]["delivery"], "delivered_unverified");
        assert_eq!(value["details"]["evidence"], "process_instance_not_running");
    }
}

#[test]
fn confirm_watches_the_process_until_it_exits_within_its_slice() {
    let _clock = VirtualClock::freeze();
    let adapter = PhasedAdapter::new(
        Lease::GrantedJustBeforeDeadline,
        Close::FailsThenExits {
            code: ErrorCode::Timeout,
            disposition: DeliverySemantics::delivered_unverified(),
            exit_after: Duration::from_millis(100),
        },
    );

    let value = execute(close_args(), &adapter).unwrap();

    assert_eq!(value["disposition"]["delivery"], "delivered_verified");
    assert_eq!(value["details"]["evidence"], "process_instance_exited");
}

#[test]
fn refusals_and_mismatches_surface_unchanged_even_when_the_process_is_gone() {
    for (code, disposition) in [
        (ErrorCode::ActionFailed, DeliverySemantics::not_delivered()),
        (ErrorCode::InvalidArgs, DeliverySemantics::not_delivered()),
        (ErrorCode::StaleRef, DeliverySemantics::unknown()),
        (ErrorCode::StaleRef, DeliverySemantics::uncertain()),
        (
            ErrorCode::PlatformNotSupported,
            DeliverySemantics::unknown(),
        ),
        (ErrorCode::AppUnresponsive, DeliverySemantics::unknown()),
    ] {
        let _clock = VirtualClock::freeze();
        let adapter = fails_then_exits(code.clone(), disposition);

        let error = adapter_error(execute(close_args(), &adapter).unwrap_err());

        assert_eq!(error.code, code);
        assert_eq!(error.disposition, disposition);
    }
}

#[test]
fn lease_timeout_is_not_delivered_and_names_its_phase_and_slice() {
    let _clock = VirtualClock::freeze();
    let adapter = PhasedAdapter::new(Lease::TimesOut, Close::ExitsAfterRequest);

    let error = adapter_error(execute(close_args(), &adapter).unwrap_err());

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    let details = error.details.unwrap();
    assert_eq!(details["phase"], "awaiting_lease");
    assert_eq!(details["timeout_ms"], 2_500);
    assert_eq!(adapter.close_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn slow_inventory_timeout_names_the_resolving_phase() {
    let _clock = VirtualClock::freeze();
    let mut adapter = PhasedAdapter::new(Lease::TimesOut, Close::ExitsAfterRequest);
    adapter.slow_inventory = true;

    let error = adapter_error(execute(close_args(), &adapter).unwrap_err());

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(error.details.unwrap()["phase"], "resolving");
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

    let error = adapter_error(execute(close_args_within(Some(2_000)), &adapter).unwrap_err());

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.unwrap();
    assert_eq!(details["phase"], "closing");
    assert_eq!(details["timeout_ms"], 1_750);
}
