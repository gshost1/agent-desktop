//! Tells a document window or dialog apart from an auxiliary window by its
//! accessibility subrole.

use agent_desktop_core::{Deadline, WindowInfo};

/// Whether `window` is auxiliary: its subrole names neither a standard
/// window nor a dialog, as for a floating panel or a borderless suggestion
/// list. A window that cannot be read is not auxiliary, so the caller keeps
/// every candidate it cannot classify.
pub(crate) fn is_auxiliary_window(window: &WindowInfo, deadline: Deadline) -> bool {
    let Ok(element) = super::window_resolve::window_element_for_info(window, deadline) else {
        return false;
    };
    let subrole = crate::tree::properties::attributes::copy_string_attr_result(
        &element,
        "AXSubrole",
        deadline,
    );
    deadline.ensure_remaining().is_ok()
        && matches!(subrole, Ok(Some(subrole)) if is_auxiliary_subrole(&subrole))
}

/// Subroles other than a standard window's or a dialog's.
pub(crate) fn is_auxiliary_subrole(subrole: &str) -> bool {
    !matches!(subrole, "AXStandardWindow" | "AXDialog" | "AXSystemDialog")
}

#[cfg(test)]
#[path = "window_subrole_tests.rs"]
mod tests;
