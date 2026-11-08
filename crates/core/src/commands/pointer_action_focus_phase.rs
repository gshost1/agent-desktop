use crate::{
    AppError,
    adapter::PlatformAdapter,
    commands::{
        helpers::load_ref_entry,
        point_resolve::{PointResolveArgs, focus_for_physical_input},
    },
    context::CommandContext,
};

/// The headed focus a pointer command runs before its first pointer event.
/// It runs once per command: the first attempt that reaches it raises the
/// target window, and every later attempt, whether the wait's scroll or a
/// retried attempt under a new lease, reuses that outcome instead of raising
/// the window again. A focus that fails leaves the phase `Unfocused` and the
/// refusal goes to the caller as it was raised.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PointerFocus<'a> {
    /// The target needs no focus: a coordinate target, or one whose headed
    /// requirement does not ask for a focused window.
    NotRequired,
    /// The target's window has not been focused in this command yet.
    Unfocused { ref_id: &'a str },
    /// The focus ran; `raised` says whether it raised a window, which it does
    /// not when the physical input policy forbids stealing focus.
    Focused { raised: bool },
}

impl<'a> PointerFocus<'a> {
    pub(crate) fn new(args: PointResolveArgs<'a>) -> Self {
        match args.ref_id {
            Some(ref_id) if args.headed_requirement.requires_focus() => Self::Unfocused { ref_id },
            _ => Self::NotRequired,
        }
    }

    /// Runs the focus if this command has not run it yet and returns whether
    /// the command raised the target window.
    pub(crate) fn ensure(
        &mut self,
        lease: &crate::InteractionLease,
        adapter: &dyn PlatformAdapter,
        context: &CommandContext,
    ) -> Result<bool, AppError> {
        let ref_id = match *self {
            Self::NotRequired => return Ok(false),
            Self::Focused { raised } => return Ok(raised),
            Self::Unfocused { ref_id } => ref_id,
        };
        let entry = load_ref_entry(ref_id, context)?;
        let raised = focus_for_physical_input(Some(&entry), adapter, context, lease)?;
        *self = Self::Focused { raised };
        Ok(raised)
    }
}

#[cfg(test)]
#[path = "pointer_action_focus_phase_tests.rs"]
mod tests;
