use crate::{
    AppError,
    commands::{helpers::load_ref_entry, point_resolve::PointResolveArgs},
    context::CommandContext,
};

/// Refuses a pointer gesture when one of its target refs was not proven unique
/// when it was captured. It runs before the command reads the application,
/// takes the interaction lease or raises a window, so a headed gesture never
/// raises a window for a ref it is about to refuse. The refusal is the one
/// resolution gives, under any timeout.
pub(crate) fn refuse_unproven_refs(
    targets: &[PointResolveArgs<'_>],
    context: &CommandContext,
) -> Result<(), AppError> {
    for ref_id in targets.iter().filter_map(|target| target.ref_id) {
        let entry = load_ref_entry(ref_id, context)?;
        if let Some(refusal) = crate::refs::resolve_errors::refused_before_reading(&entry) {
            crate::ref_action::ref_action_wait_support::trace_resolve_error(
                context, ref_id, &refusal,
            );
            return Err(refusal.into());
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "pointer_action_capture_refusal_tests.rs"]
mod tests;
