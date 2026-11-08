use super::{is_retryable_point_error, point_error_report};
use crate::{
    AppError, actionability::pointer_timeout::point_actionability_timeout,
    adapter::PlatformAdapter, commands::helpers::load_ref_entry,
    commands::point_resolve::PointResolveArgs, context::CommandContext,
    refs::ref_resolve_deadline::POLL_INTERVAL,
};

/// What one leased attempt of a pointer command did to the front window. The
/// focus runs under the attempt's own lease, next to the hit test and the
/// input it serves, and the window in front is read just before it, so the
/// outcome does not depend on how long the raise takes to land. An attempt
/// that ran no focus call stays `Unfocused`.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AttemptFocus {
    /// No focus call ran in this attempt.
    Unfocused,
    /// The target's window was already in front, so the focus raised nothing.
    AlreadyFront,
    /// Another window, or none, was in front, so the focus raised the target.
    FrontChanged,
    /// The window in front could not be read before the focus call.
    Unknown,
}

impl AttemptFocus {
    /// Focuses the target's window when headed input needs it, records which
    /// window was in front before, and returns whether a focus call raised it.
    pub(crate) fn run(
        &mut self,
        args: PointResolveArgs<'_>,
        lease: &crate::InteractionLease,
        adapter: &dyn PlatformAdapter,
        context: &CommandContext,
    ) -> Result<bool, AppError> {
        if !args.headed_requirement.requires_focus() {
            return Ok(false);
        }
        let Some(ref_id) = args.ref_id else {
            return Ok(false);
        };
        let entry = load_ref_entry(ref_id, context)?;
        let before = front_before_focus(&entry, lease, adapter, context);
        let outcome = crate::commands::point_resolve::focus_for_physical_input(
            Some(&entry),
            adapter,
            context,
            lease,
        );
        let focus_ran = match &outcome {
            Ok(raised) => *raised,
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
    entry: &crate::RefEntry,
    lease: &crate::InteractionLease,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> AttemptFocus {
    if !context.physical_input_policy().allow_focus_steal || entry.source.is_process_owned() {
        return AttemptFocus::Unfocused;
    }
    let Ok(target) = crate::saved_ref_focus_window(entry) else {
        return AttemptFocus::Unknown;
    };
    match adapter.focused_window(lease.deadline()) {
        Ok(Some(front)) if front.id == target.id && front.pid == target.pid => {
            AttemptFocus::AlreadyFront
        }
        Ok(_) => AttemptFocus::FrontChanged,
        Err(_) => AttemptFocus::Unknown,
    }
}

/// Runs a pointer command's leased attempts until one resolves. A refusal
/// that may clear goes back to the wait while the budget lasts, unless the
/// attempt's focus brought the target's window to the front: that refusal is
/// the command's result, so a command raises a window over another at most
/// once, as a ref action does. A focus that found the target already in
/// front, or could not read the window in front, retries as before. A
/// refusal from an attempt whose focus ran carries `preparatory_focus`.
pub(crate) fn retry_leased_point_phase<T>(
    timeout_ms: Option<u64>,
    deadline: crate::Deadline,
    mut attempt: impl FnMut(&mut AttemptFocus) -> Result<T, AppError>,
) -> Result<T, AppError> {
    let auto_wait = timeout_ms.is_some_and(|timeout_ms| timeout_ms > 0);
    loop {
        let mut focus = AttemptFocus::Unfocused;
        let error = match attempt(&mut focus) {
            Ok(value) => return Ok(value),
            Err(error) if focus == AttemptFocus::Unfocused => error,
            Err(AppError::Adapter(error)) => AppError::Adapter(error.after_focus_ran()),
            Err(error) => error,
        };
        if !auto_wait || focus == AttemptFocus::FrontChanged || !is_retryable_point_error(&error) {
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
