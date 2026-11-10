use crate::tree::{AXElement, surface_read};
use agent_desktop_core::Deadline;

/// Attributes that carry an item's visible label, read from an `AXOpen` target
/// before the action so the opened view's window title can be matched to it.
const LABEL_ATTRIBUTES: [&str; 3] = ["AXTitle", "AXValue", "AXDescription"];

/// The title of the window an `AXOpen` target lives in, held by the window
/// element so it can be read again after the target is gone, with the target's
/// own labels. Opening an item in place replaces the view the target lived in
/// and retitles the window after the item: Finder answers `AXOpen` on a folder
/// item with an uninformative code after it has navigated. Only a new title
/// that names the target counts, so an unrelated retitle proves nothing.
pub(crate) struct WindowTitle {
    window: AXElement,
    title: String,
    labels: Vec<String>,
}

impl WindowTitle {
    pub(crate) fn capture(element: &AXElement, action: &str, deadline: Deadline) -> Option<Self> {
        if action != "AXOpen" {
            return None;
        }
        let end = crate::tree::locator_deadline::from_operation(deadline).ok()?;
        let labels: Vec<String> = LABEL_ATTRIBUTES
            .iter()
            .filter_map(|attribute| surface_read::string(element, attribute, end).ok().flatten())
            .filter(|label| present(label))
            .collect();
        if labels.is_empty() {
            return None;
        }
        let window = surface_read::element(element, "AXWindow", end).ok()??;
        let title = surface_read::string(&window, "AXTitle", end)
            .ok()
            .flatten()
            .filter(|title| present(title))?;
        Some(Self {
            window,
            title,
            labels,
        })
    }

    pub(crate) fn changed(&self, deadline: Deadline) -> bool {
        let Ok(end) = crate::tree::locator_deadline::from_operation(deadline) else {
            return false;
        };
        let now = surface_read::string(&self.window, "AXTitle", end)
            .ok()
            .flatten();
        retitled_after_target(&self.title, now.as_deref(), &self.labels)
    }
}

fn present(text: &str) -> bool {
    !text.trim().is_empty()
}

/// The window now carries a readable title that differs from the one before
/// the action and equals one of the target's labels. A closed window, an
/// unreadable or blank title, or a title that does not name the target proves
/// nothing about the action.
fn retitled_after_target(before: &str, after: Option<&str>, labels: &[String]) -> bool {
    let Some(after) = after.map(str::trim).filter(|after| !after.is_empty()) else {
        return false;
    };
    after != before.trim() && labels.iter().any(|label| label.trim() == after)
}

#[cfg(test)]
#[path = "activation_title_tests.rs"]
mod tests;
