use super::{focus_point_under_lease, is_retryable_point_error, point_error_report};
use crate::{
    AdapterError, AppError, actionability::pointer_timeout::point_actionability_timeout,
    actions::headed_focus::same_window, adapter::PlatformAdapter,
    commands::point_resolve::PointResolveArgs, context::CommandContext,
    ref_action::ref_action_wait::with_refusal, refs::ref_resolve_deadline::POLL_INTERVAL,
};

/// What one leased attempt of a pointer command did to the front window. The
/// focus runs under the attempt's own lease, next to the hit test and the
/// input it serves, and the window in front is read just before the focus
/// call, against the exact window that call raises, so the outcome does not
/// depend on how long the raise takes to land. An attempt that ran no focus
/// call stays `Unfocused`.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AttemptFocus {
    /// No focus call ran in this attempt.
    Unfocused,
    /// The window the focus raises was already in front.
    AlreadyFront,
    /// Another window was in front, so the focus brought the target forward.
    FrontChanged,
    /// The window in front could not be read, or no window was reported in
    /// front, before the focus call.
    Unknown,
}

impl AttemptFocus {
    /// Focuses the target's window when headed input needs it, recording which
    /// window was in front just before, and returns whether a focus call ran.
    /// That is what a command reports as `focused: true`: it asked the window
    /// manager to focus the target, whether or not the target was in front.
    pub(crate) fn run(
        &mut self,
        args: PointResolveArgs<'_>,
        lease: &crate::InteractionLease,
        adapter: &dyn PlatformAdapter,
        context: &CommandContext,
    ) -> Result<bool, AppError> {
        let mut before = Self::Unfocused;
        let outcome = focus_point_under_lease(args, lease, adapter, context, &mut |target| {
            before = front_before_focus(target, lease, adapter);
        });
        let focus_ran = match &outcome {
            Ok(ran) => *ran,
            Err(AppError::Adapter(error)) => error.moved_focus(),
            Err(_) => false,
        };
        if focus_ran {
            *self = before;
        }
        outcome
    }
}

fn front_before_focus(
    target: &crate::WindowInfo,
    lease: &crate::InteractionLease,
    adapter: &dyn PlatformAdapter,
) -> AttemptFocus {
    match adapter.focused_window(lease.deadline()) {
        Ok(Some(front)) if same_window(&front, target) => AttemptFocus::AlreadyFront,
        Ok(Some(_)) => AttemptFocus::FrontChanged,
        Ok(None) | Err(_) => AttemptFocus::Unknown,
    }
}

/// Runs a pointer command's leased attempts until one resolves. A refusal
/// that may clear goes back to the wait while the budget lasts, unless the
/// attempt's focus brought the target's window in front of another: that
/// refusal is the command's result, with the refusal that sent the previous
/// attempt back to the wait as `details.refusal`, so a command raises a window
/// over another at most once, as a ref action does. A focus that found the
/// target already in front, or could not read the window in front, retries as
/// before. A refusal from an attempt whose focus ran carries
/// `preparatory_focus`.
pub(crate) fn retry_leased_point_phase<T>(
    timeout_ms: Option<u64>,
    deadline: crate::Deadline,
    mut attempt: impl FnMut(&mut AttemptFocus) -> Result<T, AppError>,
) -> Result<T, AppError> {
    let auto_wait = timeout_ms.is_some_and(|timeout_ms| timeout_ms > 0);
    let mut refusal: Option<AdapterError> = None;
    loop {
        let mut focus = AttemptFocus::Unfocused;
        let error = match attempt(&mut focus) {
            Ok(value) => return Ok(value),
            Err(error) if focus == AttemptFocus::Unfocused => error,
            Err(error) => AppError::Adapter(error.into_adapter_error().after_focus_ran()),
        };
        if focus == AttemptFocus::FrontChanged {
            return Err(with_refusal(error.into_adapter_error(), refusal.as_ref()).into());
        }
        if !auto_wait || !is_retryable_point_error(&error) {
            return Err(error);
        }
        let remaining = deadline.remaining();
        if remaining.is_zero() {
            return Err(point_actionability_timeout(Some(point_error_report(
                &error,
            ))));
        }
        refusal = Some(error.into_adapter_error());
        std::thread::sleep(POLL_INTERVAL.min(remaining));
    }
}

#[cfg(test)]
#[path = "pointer_action_focus_phase_tests.rs"]
mod tests;
