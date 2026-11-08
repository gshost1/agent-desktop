use crate::tree::{AXElement, surface_read};
use agent_desktop_core::Deadline;

/// The title of the window an `AXOpen` target lives in, held by the window
/// element so it can be read again after the target is gone. Opening an item
/// in place replaces the view the target lived in and retitles the window:
/// Finder answers `AXOpen` on a folder item with an uninformative code after
/// it has navigated, and the new title is the observable effect.
pub(crate) struct WindowTitle {
    window: AXElement,
    title: String,
}

impl WindowTitle {
    pub(crate) fn capture(element: &AXElement, action: &str, deadline: Deadline) -> Option<Self> {
        if action != "AXOpen" {
            return None;
        }
        let end = crate::tree::locator_deadline::from_operation(deadline).ok()?;
        let window = surface_read::element(element, "AXWindow", end).ok()??;
        let title = surface_read::string(&window, "AXTitle", end).ok()??;
        Some(Self { window, title })
    }

    pub(crate) fn changed(&self, deadline: Deadline) -> bool {
        let Ok(end) = crate::tree::locator_deadline::from_operation(deadline) else {
            return false;
        };
        let now = surface_read::string(&self.window, "AXTitle", end)
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
#[path = "activation_title_tests.rs"]
mod tests;
