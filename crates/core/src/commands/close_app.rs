use crate::system::{
    close_budget::CloseBudget, close_phase::ClosePhase, process_state::ProcessState,
};
use crate::{
    AdapterError, AppError, AppInfo, DeliverySemantics, ErrorCode, OperationReport,
    OperationTarget, ProcessIdentity, adapter::PlatformAdapter,
    commands::operation_response::operation_response,
};
use serde_json::{Value, json};

const CONFIRM_POLL: std::time::Duration = std::time::Duration::from_millis(25);

pub struct CloseAppArgs {
    pub app: String,
    pub force: bool,
    pub timeout_ms: Option<u64>,
}

pub fn execute(args: CloseAppArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    if adapter.is_protected_process(&args.app) {
        return Err(protected_process_error(&args.app));
    }
    let total = crate::Deadline::after(
        args.timeout_ms
            .unwrap_or(crate::DEFAULT_OPERATION_TIMEOUT_MS),
    )
    .map_err(|error| AppError::from(error.unsent()))?;
    let budget = CloseBudget::split(total);
    let expected = crate::commands::helpers::resolve_app(
        Some(&args.app),
        adapter,
        budget.deadline(ClosePhase::Resolving),
    )
    .map_err(|error| budget.fail(ClosePhase::Resolving, error))?;
    let protected = adapter.is_protected_process(&args.app)
        || adapter.is_protected_process(&expected.name)
        || expected
            .bundle_id
            .as_deref()
            .is_some_and(|bundle| adapter.is_protected_process(bundle));
    if protected {
        return Err(protected_process_error(&args.app));
    }
    let lease = adapter
        .acquire_interaction_lease(budget.deadline(ClosePhase::AwaitingLease))
        .map_err(|error| budget.fail(ClosePhase::AwaitingLease, error))?
        .with_deadline(budget.deadline(ClosePhase::Closing));
    let live =
        crate::commands::helpers::revalidate_app_for_mutation(adapter, &expected, lease.deadline())
            .map_err(|error| budget.fail(ClosePhase::Revalidating, error))?;
    let report = match adapter.close_app(&live, args.force, &lease) {
        Ok(report) => report,
        Err(failure) => confirm_exit(
            adapter,
            &live,
            failure,
            budget.deadline(ClosePhase::ConfirmingExit),
        )
        .map_err(|failure| budget.fail(ClosePhase::Closing, failure))?,
    };
    operation_response(
        &report,
        json!({
            "app": args.app,
            "method": if args.force { "force" } else { "graceful" },
        }),
    )
}

/// A close or exit wait that failed while the process is going away has
/// reached its goal. The process is watched until it is gone or the confirm
/// slice ends: gone after a request the adapter says it sent is the exit our
/// close caused; gone after a request that may or may not have been sent is
/// delivered but unverified. Refusals, identity mismatches and unsupported
/// operations surface unchanged, as does a process still running.
fn confirm_exit(
    adapter: &dyn PlatformAdapter,
    app: &AppInfo,
    failure: AdapterError,
    deadline: crate::Deadline,
) -> Result<OperationReport, AdapterError> {
    if !exit_may_settle(&failure) {
        return Err(failure);
    }
    let Some(instance) = app.process_instance.as_deref() else {
        return Err(failure);
    };
    let Ok(target) = OperationTarget::for_app(app) else {
        return Err(failure);
    };
    let process = ProcessIdentity::new(app.pid, instance);
    loop {
        match adapter.process_state(process.clone(), deadline) {
            Ok(ProcessState::Exited { .. } | ProcessState::Crashed { .. }) => {
                return Ok(gone_report(target, failure.disposition));
            }
            Ok(ProcessState::Running | ProcessState::Unresponsive) => {}
            Err(_) => return Err(failure),
        }
        let Ok(pause) = deadline.remaining_slice(CONFIRM_POLL) else {
            return Err(failure);
        };
        crate::wait::deadline::sleep(pause);
    }
}

fn exit_may_settle(failure: &AdapterError) -> bool {
    let refusal = matches!(
        failure.code,
        ErrorCode::StaleRef
            | ErrorCode::InvalidArgs
            | ErrorCode::PlatformNotSupported
            | ErrorCode::ActionNotSupported
    );
    let close_failure = matches!(failure.code, ErrorCode::Timeout | ErrorCode::ActionFailed);
    match failure.disposition {
        DeliverySemantics::NotDelivered | DeliverySemantics::DeliveredVerified => false,
        DeliverySemantics::DeliveryUncertain => !refusal,
        DeliverySemantics::DeliveredUnverified | DeliverySemantics::Unknown => close_failure,
    }
}

fn gone_report(target: OperationTarget, disposition: DeliverySemantics) -> OperationReport {
    if disposition == DeliverySemantics::DeliveredUnverified {
        OperationReport::app_exited(target)
    } else {
        OperationReport::app_gone_after_unconfirmed_request(target)
    }
}

fn protected_process_error(app: &str) -> AppError {
    AppError::invalid_input_with_suggestion(
        format!("'{app}' is a protected system process and cannot be closed"),
        "Target a regular application; session-critical processes are never closed.",
    )
}

#[cfg(test)]
#[path = "close_app_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "close_app_phase_tests.rs"]
mod phase_tests;
