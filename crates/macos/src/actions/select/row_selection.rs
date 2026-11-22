//! Settles selection-compatible steps of a row click by reading a selection change.

use agent_desktop_core::{ActionStep, AdapterError, DeliveryDisposition, ErrorCode, StepMechanism};

use crate::actions::chain::chain_delivery::DeliveryOutcome;

pub(crate) const SELECTION_POSTCONDITION: &str = "selection_postcondition";
const MAX_SELECTED_ROWS: usize = 256;

fn complete_selection_contains<T>(
    selected: &[T],
    matches_row: impl FnMut(&T) -> bool,
) -> Option<bool> {
    (selected.len() <= MAX_SELECTED_ROWS).then(|| selected.iter().any(matches_row))
}

/// The selection is read from the member's own `AXSelected` when it exposes
/// one and otherwise from the `AXSelectedRows` of the table or outline that
/// owns it. Only a member that was unselected before the click can be
/// confirmed this way: a row that already held the selection proves nothing.
fn settle_by_selection(
    before: Option<bool>,
    step: &str,
    outcome: &Result<DeliveryOutcome, AdapterError>,
    selected_after: impl FnOnce(bool) -> bool,
) -> Option<ActionStep> {
    if before != Some(false) {
        return None;
    }
    let selection_step = matches!(step, "AXConfirm" | "select_within_container");
    let poll = match outcome {
        Err(error)
            if reached_application(error)
                && (selection_step
                    || (step == "activate_descendant"
                        && error
                            .details
                            .as_ref()
                            .and_then(|details| details.get("operation"))
                            .and_then(serde_json::Value::as_str)
                            == Some("AXConfirm"))) =>
        {
            true
        }
        Ok(DeliveryOutcome::DeliveredUnverified) if selection_step || step == "CGClick" => false,
        _ => return None,
    };
    selected_after(poll).then(postcondition_step)
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
    use agent_desktop_core::{
        ActionStep, AdapterError, Deadline, actions::interaction_policy::InteractionPolicy,
    };

    use crate::actions::chain::{ChainContext, ChainDef, execute_observed_chain};
    use crate::actions::select::container_select::{SELECTED, SELECTED_ROWS};
    use crate::tree::AXElement;

    const MAX_CONTAINER_DEPTH: usize = 3;

    pub(crate) fn click_settled_by_selection(
        element: &AXElement,
        definition: &ChainDef,
        context: &ChainContext,
        policy: InteractionPolicy,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        let deadline = context.deadline;
        let before = member_selected(element, deadline);
        execute_observed_chain(element, definition, context, policy, |step, outcome| {
            super::settle_by_selection(before, step, outcome, |settle| {
                if settle {
                    crate::actions::select::selection_settle::selection_settles(deadline, || {
                        member_selected(element, deadline) == Some(true)
                    })
                } else {
                    member_selected(element, deadline) == Some(true)
                }
            })
        })
    }

    pub(crate) fn member_selected(element: &AXElement, deadline: Deadline) -> Option<bool> {
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
                    super::MAX_SELECTED_ROWS + 1,
                    deadline,
                )
                .ok()
                .flatten()
            {
                return super::complete_selection_contains(&selected, |row| {
                    crate::tree::properties::capabilities::same_element(row, element)
                });
            }
        }
        None
    }
}

pub(crate) use imp::{click_settled_by_selection, member_selected};

#[cfg(test)]
#[path = "row_selection_tests.rs"]
mod tests;
