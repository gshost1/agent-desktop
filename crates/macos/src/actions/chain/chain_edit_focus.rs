//! Gives a text field keyboard focus before its value is written.
//!
//! AppKit routes an `AXValue` write on a focused text field through the field
//! editor, so the application's own text-change handling runs: bound models,
//! linked fields and validation all see the edit. Written while the field is
//! not focused, the cell shows the new string and the application never hears
//! of it, so the value reads back as written and a later confirm uses the old
//! one. Focus moves only inside the target's window; the application is not
//! activated.
//!
//! A field bound to a document often hands its text to the model only when
//! editing ends, as Return does. After the write, a single-line field that
//! advertises `AXConfirm` is confirmed, so the document holds the new value
//! before a later save.

use agent_desktop_core::{AdapterError, Deadline};

/// Whether a value write on `role` should first take focus.
pub(crate) fn edits_through_focus(role: Option<&str>) -> bool {
    matches!(role, Some("textfield" | "combobox"))
}

#[cfg(target_os = "macos")]
pub(crate) fn focus_before_value_write(
    element: &crate::tree::AXElement,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    use crate::actions::ax_helpers;

    let role = ax_helpers::element_role(element, deadline)?;
    if !edits_through_focus(role.as_deref()) {
        return Ok(());
    }
    if crate::tree::copy_bool_attr(element, accessibility_sys::kAXFocusedAttribute, deadline)
        == Some(true)
    {
        return Ok(());
    }
    if !ax_helpers::is_attr_settable(element, accessibility_sys::kAXFocusedAttribute, deadline)? {
        return Ok(());
    }
    match ax_helpers::ax_focus_or_err(element, deadline) {
        Ok(_) => Ok(()),
        Err(_) if !deadline.is_expired() => Ok(()),
        Err(error) => Err(error),
    }
}

/// Whether a written field of this accessibility role and subrole commits its
/// edit through `AXConfirm`. A text area takes Return as a new line, and a
/// secure field's Return can submit its form, so neither is confirmed.
pub(crate) fn commits_by_confirm(ax_role: Option<&str>, ax_subrole: Option<&str>) -> bool {
    matches!(ax_role, Some("AXTextField" | "AXComboBox")) && ax_subrole != Some("AXSecureTextField")
}

/// Confirms a written single-line field so the application commits the edit.
/// Returns whether `AXConfirm` was performed.
#[cfg(target_os = "macos")]
pub(crate) fn commit_value_write(
    element: &crate::tree::AXElement,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    use crate::tree::properties::attributes::copy_string_attr_result;

    let role = copy_string_attr_result(element, accessibility_sys::kAXRoleAttribute, deadline);
    let subrole =
        copy_string_attr_result(element, accessibility_sys::kAXSubroleAttribute, deadline);
    let (Ok(role), Ok(subrole)) = (role, subrole) else {
        return Ok(false);
    };
    if !commits_by_confirm(role.as_deref(), subrole.as_deref()) {
        return Ok(false);
    }
    crate::actions::ax_helpers::try_ax_action_or_err(element, "AXConfirm", deadline)
}

#[cfg(test)]
#[path = "chain_edit_focus_tests.rs"]
mod tests;
