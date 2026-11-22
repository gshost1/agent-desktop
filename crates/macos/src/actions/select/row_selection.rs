//! Settles a click on a selectable member, such as a table or outline row,
//! by the selection it produced. A click that reached the application and
//! moved the selection onto the member did its job even when a later step in
//! the chain, such as `AXConfirm` on a label inside the row, reports no effect.

use agent_desktop_core::{ActionStep, AdapterError, DeliveryDisposition, ErrorCode, StepMechanism};

pub(crate) const SELECTION_POSTCONDITION: &str = "selection_postcondition";

/// The selection is read from the member's own `AXSelected` when it exposes
/// one and otherwise from the `AXSelectedRows` of the table or outline that
/// owns it. Only a member that was unselected before the click can be
/// confirmed this way: a row that already held the selection proves nothing.
fn settle_by_selection(
    before: Option<bool>,
    outcome: Result<Vec<ActionStep>, AdapterError>,
    mut selected_after: impl FnMut(bool) -> bool,
) -> Result<Vec<ActionStep>, AdapterError> {
    if before != Some(false) {
        return outcome;
    }
    match outcome {
        Err(error) if reached_application(&error) && selected_after(true) => {
            Ok(vec![postcondition_step()])
        }
        Ok(mut steps)
            if !steps.iter().any(|step| step.verified() == Some(true)) && selected_after(false) =>
        {
            steps.push(postcondition_step());
            Ok(steps)
        }
        other => other,
    }
}

fn reached_application(error: &AdapterError) -> bool {
    error.code == ErrorCode::ActionFailed
        && matches!(
            error.disposition.delivery(),
            DeliveryDisposition::DeliveryUncertain | DeliveryDisposition::DeliveredUnverified
        )
}

fn postcondition_step() -> ActionStep {
    ActionStep::succeeded(SELECTION_POSTCONDITION)
        .with_mechanism(StepMechanism::SemanticApi)
        .with_verified(true)
}

mod imp {
    use agent_desktop_core::{ActionStep, AdapterError, Deadline};

    use crate::actions::select::container_select::{SELECTED, element_activates_by_selection};
    use crate::tree::AXElement;

    const MAX_SELECTED_ROWS: usize = 256;
    const MAX_CONTAINER_DEPTH: usize = 3;
    const SELECTED_ROWS: &str = "AXSelectedRows";

    pub(crate) fn click_settled_by_selection(
        element: &AXElement,
        deadline: Deadline,
        click: impl FnOnce() -> Result<Vec<ActionStep>, AdapterError>,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        let before = if element_activates_by_selection(element, deadline) {
            member_selected(element, deadline)
        } else {
            None
        };
        super::settle_by_selection(before, click(), |settle| {
            if settle {
                crate::actions::select::selection_settle::selection_settles(deadline, || {
                    member_selected(element, deadline) == Some(true)
                })
            } else {
                member_selected(element, deadline) == Some(true)
            }
        })
    }

    fn member_selected(element: &AXElement, deadline: Deadline) -> Option<bool> {
        if let Some(own) =
            crate::tree::properties::attributes::copy_bool_attr_result(element, SELECTED, deadline)
                .ok()
                .flatten()
        {
            return Some(own);
        }
        container_holds(element, deadline)
    }

    fn container_holds(element: &AXElement, deadline: Deadline) -> Option<bool> {
        let mut current = element.clone();
        for _ in 0..MAX_CONTAINER_DEPTH {
            current = crate::tree::properties::attributes::copy_element_attr_result(
                &current, "AXParent", deadline,
            )
            .ok()
            .flatten()?;
            if let Some(selected) =
                crate::tree::properties::attributes::copy_ax_array_prefix_result(
                    &current,
                    SELECTED_ROWS,
                    MAX_SELECTED_ROWS,
                    deadline,
                )
                .ok()
                .flatten()
            {
                return Some(
                    selected.iter().any(|row| {
                        crate::tree::properties::capabilities::same_element(row, element)
                    }),
                );
            }
        }
        None
    }
}

pub(crate) use imp::click_settled_by_selection;

#[cfg(test)]
#[path = "row_selection_tests.rs"]
mod tests;
