use super::{focus_point_under_lease, is_retryable_point_error, point_error_report};
use crate::{
    AdapterError, AppError, ErrorCode, actionability::pointer_timeout::point_actionability_timeout,
    adapter::PlatformAdapter, commands::point_resolve::PointResolveArgs, context::CommandContext,
    ref_action::ref_action_wait::with_refusal, refs::ref_resolve_deadline::POLL_INTERVAL,
};
use serde_json::Value;

/// The headed focus of one pointer command, shared by its pre-lease wait and
/// its leased attempts. The command raises its target's window at most once:
/// after a focus call has raised it, later calls go straight to their checks
/// and input. Raising again does not uncover a target that another window
/// still covers; such a target fails the hit test until the budget ends,
/// without a raise on every poll.
#[derive(Debug, Default)]
pub(crate) struct CommandFocus {
    raised: bool,
    failed: bool,
}

impl CommandFocus {
    /// Focuses the target's window when `args` needs it for headed input and
    /// this command has not raised it yet, and returns whether the window was
    /// raised. That is what a command reports as `focused: true`.
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
        let outcome = focus_point_under_lease(args, lease, adapter, context);
        match &outcome {
            Ok(raised) => self.raised = *raised,
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
    let mut refusal: Option<AdapterError> = None;
    loop {
        let error = match attempt(focus) {
            Ok(value) => return Ok(value),
            Err(error) => error,
        };
        if focus.failed || !auto_wait {
            return Err(error);
        }
        if !is_retryable_point_error(&error) {
            return Err(with_last_refusal(error, refusal.as_ref()));
        }
        let remaining = deadline.remaining();
        if remaining.is_zero() {
            let timeout = point_actionability_timeout(Some(point_error_report(&error)));
            return Err(with_last_refusal(
                timeout,
                Some(&error.into_adapter_error()),
            ));
        }
        refusal = Some(error.into_adapter_error());
        std::thread::sleep(POLL_INTERVAL.min(remaining));
    }
}

/// Carries the refusal that sent a pointer wait's last attempt back to the
/// wait on the `TIMEOUT` that ends it, as `details.refusal`, the way a ref
/// action's wait does, and replaces the generic suggestion with one for the
/// check that refused. Any other error is returned unchanged.
pub(crate) fn with_last_refusal(error: AppError, refusal: Option<&AdapterError>) -> AppError {
    let Some(refusal) = refusal else {
        return error;
    };
    if error.code() != ErrorCode::Timeout.as_str() {
        return error;
    }
    let error = with_refusal(error.into_adapter_error(), Some(refusal));
    AppError::Adapter(match refusal_suggestion(refusal) {
        Some(suggestion) => error.with_suggestion(suggestion),
        None => error,
    })
}

fn refusal_suggestion(refusal: &AdapterError) -> Option<&'static str> {
    let check = refused_check(refusal.details.as_ref()?)?;
    match check {
        "receives_events" => Some(
            "Another element covered the target on the last attempt. Bring the target's window to the front or move what covers it, then retry.",
        ),
        "visible" => Some(
            "The target was not visible on the last attempt. Scroll it into view or restore its window, then retry.",
        ),
        "stable" => Some(
            "The target was still moving on the last attempt. Wait for it to settle, then retry.",
        ),
        _ => None,
    }
}

fn refused_check(details: &Value) -> Option<&str> {
    if let Some(check) = details.get("check").and_then(Value::as_str) {
        return Some(check);
    }
    details
        .get("checks")?
        .as_array()?
        .iter()
        .find(|check| check.get("status").and_then(Value::as_str) == Some("fail"))?
        .get("check")?
        .as_str()
}

#[cfg(test)]
#[path = "pointer_action_focus_phase_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "pointer_action_focus_phase_refusal_tests.rs"]
mod refusal_tests;
