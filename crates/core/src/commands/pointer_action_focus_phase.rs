use super::{focus_point_under_lease, is_retryable_point_error, point_error_report};
use crate::{
    AppError, actionability::pointer_timeout::point_actionability_timeout,
    adapter::PlatformAdapter, commands::point_resolve::PointResolveArgs, context::CommandContext,
    refs::ref_resolve_deadline::POLL_INTERVAL,
};

/// What one leased attempt of a pointer command did to the target's window.
/// The focus runs under the attempt's own lease, next to the hit test and the
/// input it serves, so each attempt starts `Unfocused`. An attempt that
/// raised the window, including a focus call that raised it and then failed,
/// is `Raised`, and its refusal ends the command: a command raises a window
/// at most once, as a ref action does.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AttemptFocus {
    Unfocused,
    Raised,
}

impl AttemptFocus {
    /// Focuses the target's window when headed input needs it and returns
    /// whether a window was raised.
    pub(crate) fn run(
        &mut self,
        args: PointResolveArgs<'_>,
        lease: &crate::InteractionLease,
        adapter: &dyn PlatformAdapter,
        context: &CommandContext,
    ) -> Result<bool, AppError> {
        let outcome = focus_point_under_lease(args, lease, adapter, context);
        let raised = match &outcome {
            Ok(raised) => *raised,
            Err(AppError::Adapter(error)) => error.moved_focus(),
            Err(_) => false,
        };
        if raised {
            *self = Self::Raised;
        }
        outcome
    }
}

/// Runs a pointer command's leased attempts until one resolves. A refusal
/// that may clear goes back to the wait while the budget lasts, unless the
/// attempt raised the target's window: that refusal is the command's result.
pub(crate) fn retry_leased_point_phase<T>(
    timeout_ms: Option<u64>,
    deadline: crate::Deadline,
    mut attempt: impl FnMut(&mut AttemptFocus) -> Result<T, AppError>,
) -> Result<T, AppError> {
    let auto_wait = timeout_ms.is_some_and(|timeout_ms| timeout_ms > 0);
    loop {
        let mut focus = AttemptFocus::Unfocused;
        match attempt(&mut focus) {
            Ok(value) => return Ok(value),
            Err(error)
                if auto_wait
                    && focus == AttemptFocus::Unfocused
                    && is_retryable_point_error(&error) =>
            {
                let remaining = deadline.remaining();
                if remaining.is_zero() {
                    return Err(point_actionability_timeout(Some(point_error_report(
                        &error,
                    ))));
                }
                std::thread::sleep(POLL_INTERVAL.min(remaining));
            }
            Err(error) => return Err(error),
        }
    }
}
