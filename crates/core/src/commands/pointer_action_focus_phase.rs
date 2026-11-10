use super::{focus_point_under_lease, is_retryable_point_error, point_error_report};
use crate::{
    AppError, actionability::pointer_timeout::point_actionability_timeout,
    adapter::PlatformAdapter, commands::point_resolve::PointResolveArgs, context::CommandContext,
    refs::ref_resolve_deadline::POLL_INTERVAL,
};

/// The headed focus of one pointer command. The command raises its target's
/// window at most once: after a focus call has run, a later attempt goes
/// straight to its checks and input. A raise cannot be confirmed by reading
/// the front window before it, because that read can fail in a fresh process,
/// and raising again does not uncover a target that another window still
/// covers. Such a target then fails the hit test until the budget ends, as an
/// occluded refusal or a `TIMEOUT`, without raising the window on every poll.
#[derive(Debug, Default)]
pub(crate) struct CommandFocus {
    ran: bool,
}

impl CommandFocus {
    /// Focuses the target's window when headed input needs it and no focus
    /// call has run in this command, and returns whether one has run. That is
    /// what a command reports as `focused: true`. A focus call that failed
    /// after it may have raised the window counts as run.
    pub(crate) fn run(
        &mut self,
        args: PointResolveArgs<'_>,
        lease: &crate::InteractionLease,
        adapter: &dyn PlatformAdapter,
        context: &CommandContext,
    ) -> Result<bool, AppError> {
        if self.ran {
            return Ok(true);
        }
        let outcome = focus_point_under_lease(args, lease, adapter, context);
        self.ran = match &outcome {
            Ok(ran) => *ran,
            Err(AppError::Adapter(error)) => error.moved_focus(),
            Err(_) => false,
        };
        outcome
    }

    pub(crate) fn has_run(&self) -> bool {
        self.ran
    }
}

/// Runs a pointer command's leased attempts until one resolves. A refusal
/// that may clear goes back to the wait while the budget lasts. Every refusal
/// after this command's focus call has run, and the `TIMEOUT` that ends such a
/// wait, carries `preparatory_focus`.
pub(crate) fn retry_leased_point_phase<T>(
    timeout_ms: Option<u64>,
    deadline: crate::Deadline,
    mut attempt: impl FnMut(&mut CommandFocus) -> Result<T, AppError>,
) -> Result<T, AppError> {
    let auto_wait = timeout_ms.is_some_and(|timeout_ms| timeout_ms > 0);
    let mut focus = CommandFocus::default();
    loop {
        let error = match attempt(&mut focus) {
            Ok(value) => return Ok(value),
            Err(error) if !focus.has_run() => error,
            Err(error) => AppError::Adapter(error.into_adapter_error().after_focus_ran()),
        };
        if !auto_wait || !is_retryable_point_error(&error) {
            return Err(error);
        }
        let remaining = deadline.remaining();
        if remaining.is_zero() {
            let timeout = point_actionability_timeout(Some(point_error_report(&error)));
            if !focus.has_run() {
                return Err(timeout);
            }
            return Err(AppError::Adapter(
                timeout.into_adapter_error().after_focus_ran(),
            ));
        }
        std::thread::sleep(POLL_INTERVAL.min(remaining));
    }
}

#[cfg(test)]
#[path = "pointer_action_focus_phase_tests.rs"]
mod tests;
