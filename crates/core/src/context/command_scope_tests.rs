use std::time::Duration;

use serde_json::json;

use crate::refs::refs_lock::RefStoreLock;
use crate::refs::refs_test_support::HomeGuard;
use crate::session::{SessionTraceMode, StartSessionOptions, start_session};
use crate::wait::deadline::VirtualClock;
use crate::{AdapterError, AppError, Deadline, DeliverySemantics, ErrorCode};

use super::CommandContext;

struct FullTrace(std::path::PathBuf);

impl FullTrace {
    fn new(label: &str) -> Self {
        Self(std::env::temp_dir().join(format!(
            "agent-desktop-closing-{label}-{}.jsonl",
            crate::refs::new_snapshot_id()
        )))
    }

    fn fill(&self) {
        std::fs::OpenOptions::new()
            .write(true)
            .open(&self.0)
            .unwrap()
            .set_len(crate::trace::MAX_TRACE_FILE_BYTES)
            .unwrap();
    }
}

impl Drop for FullTrace {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn closing_failure(
    command_mutates: bool,
    result: Result<serde_json::Value, AppError>,
) -> AdapterError {
    let trace = FullTrace::new("closing");
    let context = CommandContext::new(None, Some(trace.0.clone()), true).unwrap();
    let scope = if command_mutates {
        context.mutating_command_scope("click").unwrap()
    } else {
        context.command_scope("snapshot").unwrap()
    };
    trace.fill();
    match scope.complete(&result) {
        Ok(()) => panic!("a full strict trace must fail the closing record"),
        Err(AppError::Adapter(error)) => error,
        Err(other) => panic!("a closing failure must be an adapter error, got {other:?}"),
    }
}

fn unknown_timeout() -> AppError {
    AdapterError::new(ErrorCode::Timeout, "the mutation did not settle in time").into()
}

#[test]
fn a_full_recording_after_an_undecided_mutation_is_uncertain_not_an_argument_refusal() {
    let error = closing_failure(true, Err(unknown_timeout()));

    assert_eq!(error.code, ErrorCode::InvalidArgs);
    assert_eq!(error.disposition, DeliverySemantics::uncertain());
}

#[test]
fn an_error_without_delivery_evidence_after_a_mutation_is_uncertain() {
    let result = Err(AppError::Internal("the adapter stopped answering".into()));

    let error = closing_failure(true, result);

    assert_eq!(error.disposition, DeliverySemantics::uncertain());
}

#[test]
fn an_argument_refusal_raised_before_anything_ran_stays_not_delivered() {
    let refusal = AdapterError::new(ErrorCode::InvalidArgs, "bad ref").into();

    let error = closing_failure(true, Err(refusal));

    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn an_argument_refusal_after_a_focus_call_is_uncertain() {
    let refusal = AdapterError::new(ErrorCode::InvalidArgs, "bad key")
        .with_details(json!({ "preparatory_focus": "may_have_moved_focus" }))
        .into();

    let error = closing_failure(true, Err(refusal));

    assert_eq!(error.disposition, DeliverySemantics::uncertain());
}

#[test]
fn a_reported_delivery_survives_the_closing_failure() {
    let verified = json!({ "disposition": DeliverySemantics::delivered_verified() });
    let unverified = AdapterError::new(ErrorCode::ActionFailed, "no postcondition")
        .with_disposition(DeliverySemantics::delivered_unverified())
        .into();

    assert_eq!(
        closing_failure(true, Ok(verified)).disposition,
        DeliverySemantics::delivered_verified()
    );
    assert_eq!(
        closing_failure(true, Err(unverified)).disposition,
        DeliverySemantics::delivered_unverified()
    );
    assert_eq!(
        closing_failure(true, Ok(json!({}))).disposition,
        DeliverySemantics::delivered_unverified()
    );
}

#[test]
fn a_read_only_command_leaves_an_undecided_delivery_to_its_effect() {
    let error = closing_failure(false, Err(unknown_timeout()));

    assert_eq!(error.disposition, DeliverySemantics::unknown());
}

#[test]
fn a_closing_record_that_waits_out_a_held_budget_lock_keeps_the_mutation_unsafe() {
    let _home = HomeGuard::new();
    let session = start_session(StartSessionOptions {
        trace: SessionTraceMode::On,
        ..Default::default()
    })
    .unwrap();
    let context = CommandContext::new(Some(session.id.clone()), None, true).unwrap();
    let scope = context.mutating_command_scope("click").unwrap();
    let trace_dir = crate::session::trace_dir(&session.id).unwrap();
    let _held = RefStoreLock::acquire_with_deadline(
        &trace_dir.join(".trace-budget.lock"),
        Deadline::detached_after(1_000).unwrap(),
    )
    .unwrap();
    let clock = VirtualClock::freeze();

    let error = match scope.complete(&Err(unknown_timeout())) {
        Ok(()) => panic!("a held budget lock must fail the strict closing record"),
        Err(AppError::Adapter(error)) => error,
        Err(other) => panic!("a closing failure must be an adapter error, got {other:?}"),
    };

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.details.as_ref().unwrap()["kind"], "lock_timeout");
    assert_eq!(error.disposition, DeliverySemantics::uncertain());
    assert!(clock.elapsed() >= Duration::from_millis(250));
}
