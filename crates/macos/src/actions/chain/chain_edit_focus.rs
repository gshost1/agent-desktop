//! Gives a text field keyboard focus before its value is written.
//!
//! AppKit routes an `AXValue` write on a focused text field through the field
//! editor, so the application's own text-change handling runs: bound models,
//! linked fields and validation all see the edit. Written while the field is
//! not focused, the cell shows the new string and the application never hears
//! of it, so the value reads back as written and a later confirm uses the old
//! one. Focus moves only inside the target's window; the application is not
//! activated.

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};

use crate::actions::DeliveryTracker;

/// Whether a value write on `role` should first take focus.
pub(crate) fn edits_through_focus(role: Option<&str>) -> bool {
    matches!(role, Some("textfield" | "combobox"))
}

/// Returns what the focus step may already have changed, so a value write
/// refused after it does not report the action as never delivered.
#[cfg(target_os = "macos")]
pub(crate) fn focus_before_value_write(
    element: &crate::tree::AXElement,
    deadline: Deadline,
) -> Result<DeliveryTracker, AdapterError> {
    use crate::actions::ax_helpers;

    let role = ax_helpers::element_role(element, deadline)?;
    if !edits_through_focus(role.as_deref()) {
        return Ok(DeliveryTracker::default());
    }
    if crate::tree::copy_bool_attr(element, accessibility_sys::kAXFocusedAttribute, deadline)
        == Some(true)
    {
        return Ok(DeliveryTracker::default());
    }
    if !ax_helpers::is_attr_settable(element, accessibility_sys::kAXFocusedAttribute, deadline)? {
        return Ok(DeliveryTracker::default());
    }
    match ax_helpers::ax_focus_or_err(element, deadline) {
        Ok(_) => Ok(DeliveryTracker::default()),
        Err(error) => settle_focus_failure(error, deadline.is_expired()),
    }
}

/// A field that declines focus can still take the value, so only that
/// refusal is passed over. A lost permission, a vanished element or an
/// expired deadline ends the write, and a focus whose delivery is uncertain
/// counts as delivered.
pub(crate) fn settle_focus_failure(
    error: AdapterError,
    expired: bool,
) -> Result<DeliveryTracker, AdapterError> {
    let ends_the_write = expired
        || matches!(
            error.code,
            ErrorCode::PermDenied
                | ErrorCode::StaleRef
                | ErrorCode::ElementNotFound
                | ErrorCode::AppNotFound
                | ErrorCode::AppUnresponsive
                | ErrorCode::Timeout
        );
    if ends_the_write {
        return Err(error);
    }
    let mut focus = DeliveryTracker::default();
    if error.disposition != DeliverySemantics::not_delivered() {
        focus.mark_delivered();
    }
    Ok(focus)
}

#[cfg(test)]
#[path = "chain_edit_focus_tests.rs"]
mod tests;
