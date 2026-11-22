use crate::tree::AXElement;
use agent_desktop_core::Deadline;

pub(crate) struct ActivationState {
    pub(crate) focus: Option<crate::actions::activation::activation_effect::FocusState>,
    pub(crate) sheet: Option<crate::actions::activation::activation_sheet::OwnedSheet>,
    pub(crate) title: Option<crate::actions::activation::activation_title::WindowTitle>,
}

impl ActivationState {
    pub(crate) fn capture(element: &AXElement, action: &str, deadline: Deadline) -> Self {
        Self {
            focus: crate::actions::activation::activation_effect::focus_state(
                element,
                deadline,
                selection_verifies_activation(action),
            ),
            sheet: crate::actions::activation::activation_sheet::OwnedSheet::capture(
                element, action, deadline,
            ),
            title: crate::actions::activation::activation_title::WindowTitle::capture(
                element, action, deadline,
            ),
        }
    }

    /// True when any baseline was captured, so a settle poll can observe something.
    pub(crate) fn observable(&self) -> bool {
        self.focus.is_some() || self.sheet.is_some() || self.title.is_some()
    }

    /// True when a baseline that survives the target's removal was captured.
    pub(crate) fn surface_observable(&self) -> bool {
        self.sheet.is_some() || self.title.is_some()
    }

    /// The target's sheet closed or its window took the target's name as its title.
    pub(crate) fn surface_changed(&self, deadline: Deadline) -> bool {
        self.sheet
            .as_ref()
            .is_some_and(|sheet| sheet.closed(deadline))
            || self
                .title
                .as_ref()
                .is_some_and(|title| title.changed(deadline))
    }
}

pub(crate) fn selection_verifies_activation(action: &str) -> bool {
    !matches!(action, "AXOpen" | "AXPress")
}

#[cfg(test)]
mod tests {
    #[test]
    fn opening_or_pressing_is_not_verified_by_selection_alone() {
        assert!(!super::selection_verifies_activation("AXOpen"));
        assert!(!super::selection_verifies_activation("AXPress"));
        assert!(super::selection_verifies_activation("AXConfirm"));
    }
}
