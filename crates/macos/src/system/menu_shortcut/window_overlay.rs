use agent_desktop_core::AdapterError;
use std::time::Instant;

use crate::tree::{AXElement, surface_read};

/// Reports whether the application's focused window is, or directly holds, a
/// sheet, dialog, alert or popover. AppKit offers a chord to the key window
/// before the menu bar, and such a surface often answers chords the menu bar
/// maps to something else, so a menu press would run the wrong command.
pub(crate) fn focused_window_has_overlay(
    app: &AXElement,
    deadline: Instant,
) -> Result<bool, AdapterError> {
    let Some(window) = surface_read::element(app, "AXFocusedWindow", deadline)? else {
        return Ok(false);
    };
    if is_overlay_element(&window, deadline)?
        || surface_read::boolean(&window, "AXModal", deadline)? == Some(true)
    {
        return Ok(true);
    }
    for child in surface_read::elements(&window, "AXChildren", deadline)? {
        if is_overlay_element(&child, deadline)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn is_overlay_element(element: &AXElement, deadline: Instant) -> Result<bool, AdapterError> {
    let role = surface_read::string(element, "AXRole", deadline)?;
    if is_overlay_role(role.as_deref(), None) {
        return Ok(true);
    }
    let subrole = surface_read::string(element, "AXSubrole", deadline)?;
    Ok(is_overlay_role(role.as_deref(), subrole.as_deref()))
}

pub(crate) fn is_overlay_role(role: Option<&str>, subrole: Option<&str>) -> bool {
    matches!(role, Some("AXSheet" | "AXPopover"))
        || matches!(
            subrole,
            Some("AXSheet" | "AXDialog" | "AXSystemDialog" | "AXAlert")
        )
}

#[cfg(test)]
#[path = "window_overlay_tests.rs"]
mod tests;
