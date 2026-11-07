//! The title of the window an action's target lives in, held by the window
//! element so it can be read again after the target is gone.

use agent_desktop_core::Deadline;

use crate::tree::AXElement;
use crate::tree::properties::attributes::{copy_element_attr_result, copy_string_attr_result};

/// An action that opens an item in place replaces the view its target lived
/// in and retitles the window: Finder answers `AXOpen` on a folder item with an
/// uninformative code after it has navigated. The new title is the observable
/// effect. The window element is held because the target no longer exists.
pub(crate) struct WindowTitle {
    window: AXElement,
    title: String,
}

impl WindowTitle {
    pub(crate) fn read(element: &AXElement, deadline: Deadline) -> Option<Self> {
        let window = copy_element_attr_result(element, "AXWindow", deadline).ok()??;
        let title = copy_string_attr_result(&window, "AXTitle", deadline).ok()??;
        Some(Self { window, title })
    }

    pub(crate) fn changed(&self, deadline: Deadline) -> bool {
        let now = copy_string_attr_result(&self.window, "AXTitle", deadline)
            .ok()
            .flatten();
        title_changed(&self.title, now.as_deref())
    }
}

/// Only two readable, non-blank titles that differ count: a closed window, an
/// unreadable title or a blank one proves nothing about the action.
fn title_changed(before: &str, after: Option<&str>) -> bool {
    let present = |title: &str| !title.trim().is_empty();
    present(before) && after.is_some_and(|after| present(after) && after != before)
}

#[cfg(test)]
#[path = "window_title_tests.rs"]
mod tests;
