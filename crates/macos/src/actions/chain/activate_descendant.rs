use crate::actions::chain::chain_delivery::DeliveryOutcome;
use agent_desktop_core::AdapterError;

const LABEL_ROLES: [&str; 4] = ["AXTextField", "AXTextArea", "AXStaticText", "AXImage"];

/// A label inside a selectable member is its text, not a control: its
/// `AXConfirm` commits an edit and never activates the row around it.
fn is_member_label(selectable: bool, role: Option<&str>) -> bool {
    selectable && role.is_some_and(|role| LABEL_ROLES.contains(&role))
}

fn activate_candidates<T>(
    children: &[T],
    mut perform: impl FnMut(&T, &str) -> Result<DeliveryOutcome, AdapterError>,
) -> Result<DeliveryOutcome, AdapterError> {
    for child in children {
        for action in crate::tree::properties::action_list::PRIMARY_ACTIVATION_ACTIONS {
            let outcome = perform(child, action)?;
            if outcome.terminates_chain() {
                return Ok(outcome);
            }
        }
    }
    Ok(DeliveryOutcome::NotDelivered)
}

mod imp {
    use agent_desktop_core::{AdapterError, Deadline};

    use crate::actions::chain::chain_delivery::DeliveryOutcome;
    use crate::actions::select::container_select::{
        is_selection_row, read_selection_role, role_activates_by_selection,
    };
    use crate::tree::AXElement;

    const MAX_CANDIDATES: usize = 4;

    /// A row often publishes no activation itself while the cell inside it
    /// does: Finder's sidebar `treeitem` is inert and its `cell` carries
    /// `AXOpen`. Without this the chain falls through to writing selection,
    /// which the row accepts and reports back while the application never
    /// navigates.
    ///
    /// A member that only its container can select is chosen, not pressed
    /// through a control that happens to sit inside it.
    ///
    /// Unverified delivery ends the search too: absence of an observed effect
    /// cannot authorize another mutation on the same child or a sibling.
    pub(crate) fn activate_descendant(
        element: &AXElement,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        let instant = crate::tree::locator_deadline::from_operation(deadline)?;
        let role = read_selection_role(element, deadline);
        let selectable = role.as_deref().is_some_and(is_selection_row);
        if role.as_deref().is_some_and(|role| {
            role_activates_by_selection(crate::tree::roles::ax_role_to_str(role))
        }) && crate::tree::properties::selection_owner::selects_only_through_container(
            element, instant,
        ) {
            return Ok(DeliveryOutcome::NotDelivered);
        }
        let mut children = crate::tree::properties::attributes::copy_ax_array_prefix_result(
            element,
            "AXChildren",
            MAX_CANDIDATES,
            instant,
        )
        .ok()
        .flatten()
        .unwrap_or_default();
        if selectable {
            children.retain(|child| {
                !super::is_member_label(
                    selectable,
                    crate::tree::properties::attributes::copy_string_attr_result(
                        child, "AXRole", instant,
                    )
                    .ok()
                    .flatten()
                    .as_deref(),
                )
            });
        }

        super::activate_candidates(&children, |child, action| {
            crate::actions::ax_helpers::perform_observed_action(child, action, deadline)
        })
    }
}

pub(crate) use imp::activate_descendant;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_label_inside_a_selectable_member_is_not_an_activation_candidate() {
        for role in ["AXTextField", "AXTextArea", "AXStaticText", "AXImage"] {
            assert!(is_member_label(true, Some(role)));
        }
        for role in ["AXCell", "AXButton", "AXGroup", "AXCheckBox"] {
            assert!(!is_member_label(true, Some(role)));
        }
        assert!(!is_member_label(true, None));
    }

    #[test]
    fn a_pressable_image_remains_an_activation_candidate_outside_a_selectable_row() {
        for role in ["AXCell", "AXListItem", "AXOption", "AXTab", "AXGroup"] {
            let selectable = crate::actions::select::container_select::is_selection_row(role);
            assert!(!is_member_label(selectable, Some("AXImage")));
        }
        for role in ["AXRow", "AXOutlineRow"] {
            let selectable = crate::actions::select::container_select::is_selection_row(role);
            assert!(is_member_label(selectable, Some("AXImage")));
        }
    }

    #[test]
    fn unverified_descendant_delivery_stops_before_another_action_or_sibling() {
        let mut attempts = Vec::new();
        let outcome = activate_candidates(&[1, 2], |child, action| {
            attempts.push((*child, action.to_owned()));
            Ok(if attempts.len() == 1 {
                DeliveryOutcome::NotDelivered
            } else {
                DeliveryOutcome::DeliveredUnverified
            })
        })
        .unwrap();
        assert_eq!(outcome, DeliveryOutcome::DeliveredUnverified);
        assert_eq!(attempts, [(1, "AXPress".into()), (1, "AXOpen".into())]);
    }

    #[test]
    fn unsupported_descendant_actions_can_reach_a_verified_sibling() {
        let mut attempts = 0;
        let outcome = activate_candidates(&[1, 2, 3], |child, _| {
            attempts += 1;
            Ok(if *child == 2 {
                DeliveryOutcome::DeliveredVerified
            } else {
                DeliveryOutcome::NotDelivered
            })
        })
        .unwrap();
        assert_eq!(outcome, DeliveryOutcome::DeliveredVerified);
        assert_eq!(attempts, 4);
    }
}
