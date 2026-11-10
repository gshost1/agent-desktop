use super::{focus_point_under_lease, is_retryable_point_error, point_error_report};
use crate::{
    AppError, actionability::pointer_timeout::point_actionability_timeout,
    adapter::PlatformAdapter, commands::point_resolve::PointResolveArgs, context::CommandContext,
    refs::ref_resolve_deadline::POLL_INTERVAL,
};

/// The headed focus of one pointer command, shared by its pre-lease wait and
/// its leased attempts. The command raises its target's window at most once:
/// after a focus call has raised it, later calls go straight to their checks
/// and input. A raise cannot be confirmed by reading the front window before
/// it, because that read can fail in a fresh process, and raising again does
/// not uncover a target that another window still covers. Such a target fails
/// the hit test until the budget ends, without a raise on every poll.
#[derive(Debug, Default)]
pub(crate) struct CommandFocus {
    raised: bool,
    failed: bool,
    not_needed: bool,
}

impl CommandFocus {
    /// Focuses the target's window when headed input needs it and this
    /// command has not raised it yet, and returns whether the window was
    /// raised. That is what a command reports as `focused: true`. A command
    /// whose input needs no focus decides that once.
    pub(crate) fn run(
        &mut self,
        args: PointResolveArgs<'_>,
        lease: &crate::InteractionLease,
        adapter: &dyn PlatformAdapter,
        context: &CommandContext,
    ) -> Result<bool, AppError> {
        if self.raised {
            return Ok(true);
        }
        if self.not_needed {
            return Ok(false);
        }
        let outcome = focus_point_under_lease(args, lease, adapter, context);
        match &outcome {
            Ok(true) => self.raised = true,
            Ok(false) => self.not_needed = true,
            Err(AppError::Adapter(error)) if error.moved_focus() => self.failed = true,
            Err(_) => {}
        }
        outcome
    }

    /// Runs the phase before a pointer command's first input event and marks
    /// any error it returns with `preparatory_focus` once a focus call of this
    /// command has raised the window or failed after it may have.
    pub(crate) fn marking<T>(
        &mut self,
        phase: impl FnOnce(&mut Self) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        phase(self).map_err(|error| {
            if self.raised || self.failed {
                AppError::Adapter(error.into_adapter_error().after_focus_ran())
            } else {
                error
            }
        })
    }
}

/// Runs a pointer command's leased attempts until one resolves. A refusal
/// that may clear goes back to the wait while the budget lasts. A focus call
/// that failed after it may have raised the window ends the command, because
/// trying again would raise the window a second time.
pub(crate) fn retry_leased_point_phase<T>(
    timeout_ms: Option<u64>,
    deadline: crate::Deadline,
    focus: &mut CommandFocus,
    mut attempt: impl FnMut(&mut CommandFocus) -> Result<T, AppError>,
) -> Result<T, AppError> {
    let auto_wait = timeout_ms.is_some_and(|timeout_ms| timeout_ms > 0);
    loop {
        let error = match attempt(focus) {
            Ok(value) => return Ok(value),
            Err(error) => error,
        };
        if focus.failed || !auto_wait || !is_retryable_point_error(&error) {
            return Err(error);
        }
        let remaining = deadline.remaining();
        if remaining.is_zero() {
            return Err(point_actionability_timeout(Some(point_error_report(
                &error,
            ))));
        }
        std::thread::sleep(POLL_INTERVAL.min(remaining));
    }
}

#[cfg(test)]
#[path = "pointer_action_focus_phase_tests.rs"]
mod tests;
