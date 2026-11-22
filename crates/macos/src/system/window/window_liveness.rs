//! Whether a CoreGraphics window its application no longer lists is closed.
//!
//! An application drops a window from `AXWindows` both when it orders the
//! window out and when the window moves to another Space or the desktop sits
//! behind a full-screen Space; CoreGraphics lists both off screen. Only an
//! ordered-out window belongs to no Space, so a window counts as closed only
//! when it is off screen, missing from a complete `AXWindows` read, and the
//! window server places it on no Space. Any unknown keeps it present.

use agent_desktop_core::AdapterError;

use crate::system::window::cg_window::WindowRecord;
use crate::system::window::window_ax_state::WindowAxState;

pub(crate) fn require_live_window(
    id: &str,
    record: &WindowRecord,
    state: &WindowAxState,
    is_on_any_space: impl FnOnce(i64) -> Option<bool>,
) -> Result<(), AdapterError> {
    if is_ordered_out(record, state, is_on_any_space) {
        return Err(crate::system::window::window_resolve_errors::window_not_found(id));
    }
    Ok(())
}

fn is_ordered_out(
    record: &WindowRecord,
    state: &WindowAxState,
    is_on_any_space: impl FnOnce(i64) -> Option<bool>,
) -> bool {
    !record.presence.is_onscreen()
        && state.window_numbers_complete
        && !state.accessible_window_ids.contains(&record.window_number)
        && is_on_any_space(record.window_number) == Some(false)
}

#[cfg(test)]
#[path = "window_liveness_tests.rs"]
mod tests;
