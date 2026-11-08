use crate::system::{
    close_budget::CloseBudget, close_phase::ClosePhase, process_state::ProcessState,
};
use crate::{
    AdapterError, AppError, AppInfo, DeliverySemantics, OperationReport, ProcessIdentity,
    adapter::PlatformAdapter, commands::operation_response::operation_response,
};
use serde_json::{Value, json};

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
    .map_err(|error| ClosePhase::Resolving.settle(error))?;
    let budget = CloseBudget::split(total);
    let expected = crate::commands::helpers::resolve_app(
        Some(&args.app),
        adapter,
        budget.deadline(ClosePhase::Resolving),
    )
    .map_err(|error| ClosePhase::Resolving.settle(error))?;
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
        .map_err(|error| ClosePhase::AwaitingLease.settle(error))?
        .with_deadline(budget.deadline(ClosePhase::Closing));
    let live =
        crate::commands::helpers::revalidate_app_for_mutation(adapter, &expected, lease.deadline())
            .map_err(|error| ClosePhase::Revalidating.settle(error))?;
    let report = match adapter.close_app(&live, args.force, &lease) {
        Ok(report) => report,
        Err(error) => confirm_exit(
            adapter,
            &live,
            error,
            budget.deadline(ClosePhase::ConfirmingExit),
        )?,
    };
    operation_response(
        &report,
        json!({
            "app": args.app,
            "method": if args.force { "force" } else { "graceful" },
        }),
    )
}

/// A close that failed while the process is no longer running has reached
/// its goal. The process is read once more: when it is gone, the failure is
/// replaced by the exit our request caused, or by "already gone" when the
/// adapter cannot say its request was sent. A running process, or a read
/// that cannot answer, keeps the original failure.
fn confirm_exit(
    adapter: &dyn PlatformAdapter,
    app: &AppInfo,
    failure: AdapterError,
    deadline: crate::Deadline,
) -> Result<OperationReport, AdapterError> {
    let Some(instance) = app.process_instance.as_deref() else {
        return Err(failure);
    };
    match adapter.process_state(ProcessIdentity::new(app.pid, instance), deadline) {
        Ok(ProcessState::Exited { .. } | ProcessState::Crashed { .. }) => {
            OperationReport::from_close_dispatch(app, Ok(request_was_sent(failure.disposition)))
        }
        Ok(ProcessState::Running | ProcessState::Unresponsive) | Err(_) => Err(failure),
    }
}

fn request_was_sent(disposition: DeliverySemantics) -> bool {
    matches!(
        disposition,
        DeliverySemantics::DeliveredUnverified | DeliverySemantics::DeliveredVerified
    )
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
