//! Says whether an application has taken a value written into a text element.
//!
//! A single-line AppKit field writes its text to the application's model only
//! when editing ends, so the value reads back as written while a Save still
//! writes the old one. A text view can do the same, and not every document
//! behind one takes an accessibility write when editing ends, so only the
//! saved result settles it. A search field filters on the write itself, and a
//! secure field cannot be read, so neither is reported as waiting for a
//! commit. The rule keys on the raw accessibility role and subrole, never on
//! the application. It applies to every value write that reached the element
//! through the accessibility API, `set-value` and `clear` alike; a clear sent
//! as keystrokes goes through the field's own editor and is judged by its
//! read-back like typed text.

use agent_desktop_core::{
    Action, ActionResult, ActionStep, ActionStepOutcome, Deadline, StepMechanism,
};

const FIELD_HINT: &str = "The field shows the new text, but an app may take it only when editing ends. Before you save or confirm, run `focus` on another field in the same window to end the edit, then check the result. Do not press Return in the field: it can submit a form or press a default button.";

const TEXT_VIEW_HINT: &str = "The text view shows the new text, but the app's document may still hold the old text. Running `focus` on another field ends the edit, which commits some text views and not others, so after you save, check the saved result before you report success. Do not press Return in the text view: it inserts a line break.";

const UNREAD_HINT: &str = "The element's role could not be read after the write, so whether the app took the value is unknown. Check the result the app reports, such as a saved file, before you report success.";

/// How a value written into an element reaches the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditCommit {
    /// The write is the effect: the read-back settles it.
    Immediate,
    /// A single-line field that commits when editing ends.
    OnEndEditing,
    /// A text view whose document may take the write only when editing ends, or never.
    TextView,
    /// The role could not be read, so the commit cannot be judged.
    Unread,
}

impl EditCommit {
    pub(crate) fn from_roles(role: Option<&str>, subrole: Option<&str>) -> Self {
        match (role, subrole) {
            (Some("AXTextField"), Some("AXSearchField" | "AXSecureTextField")) => Self::Immediate,
            (Some("AXTextField"), _) => Self::OnEndEditing,
            (Some("AXTextArea"), _) => Self::TextView,
            _ => Self::Immediate,
        }
    }

    pub(crate) fn hint(self) -> Option<&'static str> {
        match self {
            Self::Immediate => None,
            Self::OnEndEditing => Some(FIELD_HINT),
            Self::TextView => Some(TEXT_VIEW_HINT),
            Self::Unread => Some(UNREAD_HINT),
        }
    }
}

/// True when a step wrote the value through the accessibility API.
pub(crate) fn wrote_value_semantically(steps: &[ActionStep]) -> bool {
    steps.iter().any(|step| {
        matches!(step.outcome, ActionStepOutcome::Succeeded)
            && step.mechanism() == Some(StepMechanism::SemanticApi)
    })
}

/// The result of a value write, marked as waiting for the application's
/// commit when the write went through the accessibility API and the
/// element's role says the app may take it only when editing ends.
pub(crate) fn value_write_result(
    action: &Action,
    steps: Vec<ActionStep>,
    read_commit: impl FnOnce() -> EditCommit,
) -> ActionResult {
    let hint = wrote_value_semantically(&steps)
        .then(read_commit)
        .and_then(EditCommit::hint);
    let result = ActionResult::from_execution(action, steps);
    match hint {
        Some(hint) => result.with_commit_unobserved(hint),
        None => result,
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn read_edit_commit(element: &crate::tree::AXElement, deadline: Deadline) -> EditCommit {
    let read = |attribute: &str| {
        let read_deadline = crate::tree::properties::ax_ipc::AxDeadline::absolute(deadline).ok()?;
        crate::tree::properties::attributes::copy_string_attr_result(
            element,
            attribute,
            read_deadline,
        )
        .ok()
    };
    let Some(role) = read(accessibility_sys::kAXRoleAttribute) else {
        return EditCommit::Unread;
    };
    let subrole = read(accessibility_sys::kAXSubroleAttribute).flatten();
    EditCommit::from_roles(role.as_deref(), subrole.as_deref())
}

#[cfg(test)]
#[path = "edit_commit_tests.rs"]
mod tests;
