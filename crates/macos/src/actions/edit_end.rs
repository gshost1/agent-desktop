//! Ends the edit a value write left open in a single-line text field, so the
//! application takes the value now rather than whenever focus next moves.
//!
//! An AppKit field hands its text to the application's model only when
//! editing ends. Neither `AXConfirm` nor clearing `AXFocused` ends it: the
//! first is the field's own action, which Return also sends and which can
//! submit a form, and the second is refused. Focus moving to another element
//! does end it, so focus goes to the nearest other plain text field in the
//! same window, which takes focus without acting, and then back to the
//! written field. Buttons, menus and every other control that acts on focus
//! are never focused. A window with no such field keeps the edit open and
//! the result says how to end it. Values of the field's neighbours, read
//! before and after, show what the commit changed.

use agent_desktop_core::{Action, ActionResult, ActionStep, Deadline, StepMechanism};
use serde_json::{Value, json};

const MAX_PEER_NODES: usize = 400;
const MAX_NEIGHBOUR_VALUES: usize = 64;
const MAX_REPORTED_CHANGES: usize = 3;

/// What ending the edit came to.
pub(crate) enum EndedEdit {
    /// Focus left the field and came back: the app has had the value.
    Committed(Value),
    /// Focus could not leave the field: the app kept the edit open, usually
    /// with an alert saying it rejected the value.
    Refused,
    /// No edit to end, or no other field to move focus to.
    NotAttempted,
}

const REFUSED_SUGGESTION: &str = "The app would not end the edit, which it does when it rejects the value; it may be showing an alert. Run `snapshot --app <App> --surface alert -i`, answer the alert (a Discard or Revert button restores the old value), then write a value the field accepts, in its range and format.";

/// The result of a `set-value` on a text field: committed when the edit
/// could be ended, refused when the app would not let it end, otherwise
/// marked as waiting for the commit.
#[cfg(target_os = "macos")]
pub(crate) fn set_value_result(
    action: &Action,
    field: &crate::tree::AXElement,
    mut steps: Vec<ActionStep>,
    deadline: Deadline,
) -> Result<ActionResult, agent_desktop_core::AdapterError> {
    use crate::actions::edit_commit::{self, EditCommit};
    if !edit_commit::wrote_value_semantically(&steps) {
        return Ok(ActionResult::from_execution(action, steps));
    }
    let commit = edit_commit::read_edit_commit(field, deadline);
    let ended = if commit == EditCommit::OnEndEditing {
        imp::end_edit(field, deadline)
    } else {
        EndedEdit::NotAttempted
    };
    match ended {
        EndedEdit::Committed(evidence) => {
            steps.push(
                ActionStep::succeeded("EndEditingByFocus")
                    .with_mechanism(StepMechanism::SemanticApi)
                    .with_verified(true),
            );
            Ok(ActionResult::from_execution(action, steps).with_commit_ended(evidence))
        }
        EndedEdit::Refused => Err(refused_commit_error()),
        EndedEdit::NotAttempted => Ok(edit_commit::value_write_result(action, steps, || commit)),
    }
}

pub(crate) fn refused_commit_error() -> agent_desktop_core::AdapterError {
    agent_desktop_core::AdapterError::new(
        agent_desktop_core::ErrorCode::ActionFailed,
        "The value was written, but the app refused to end the edit, so it did not take the value",
    )
    .with_suggestion(REFUSED_SUGGESTION)
    .with_details(json!({ "kind": "edit_commit_refused", "application_commit": "refused" }))
    .with_disposition(agent_desktop_core::DeliverySemantics::delivered_unverified())
}

/// Whether focusing an element ends another field's edit without acting:
/// a plain or search text field that is not secure and not disabled.
pub(crate) fn takes_focus_without_acting(
    role: Option<&str>,
    subrole: Option<&str>,
    enabled: Option<bool>,
) -> bool {
    role == Some("AXTextField") && subrole != Some("AXSecureTextField") && enabled != Some(false)
}

/// The nearest element `is_peer` accepts: the field's parent's subtree first,
/// then each further ancestor's, breadth first, never inside the field.
pub(crate) fn nearest_peer<T>(
    ancestors: &[T],
    is_field: impl Fn(&T) -> bool,
    mut children: impl FnMut(&T) -> Vec<T>,
    mut is_peer: impl FnMut(&T) -> bool,
) -> Option<T>
where
    T: Clone,
{
    let mut visited = 0_usize;
    for ancestor in ancestors {
        let mut pending = std::collections::VecDeque::from(children(ancestor));
        while let Some(node) = pending.pop_front() {
            visited += 1;
            if visited > MAX_PEER_NODES {
                return None;
            }
            if is_field(&node) {
                continue;
            }
            if is_peer(&node) {
                return Some(node);
            }
            pending.extend(children(&node));
        }
    }
    None
}

/// Neighbour values that changed between two reads, at most
/// [`MAX_REPORTED_CHANGES`] of them.
pub(crate) fn changed_values(
    before: &[(String, Option<String>)],
    after: &[Option<String>],
) -> Vec<Value> {
    before
        .iter()
        .zip(after)
        .filter(|((_, old), new)| old != *new)
        .take(MAX_REPORTED_CHANGES)
        .map(|((role, old), new)| json!({ "role": role, "before": old, "after": new }))
        .collect()
}

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use crate::actions::ax_helpers;
    use crate::tree::{AXElement, surface_read};
    use accessibility_sys::kAXFocusedAttribute;

    pub(super) fn end_edit(field: &AXElement, deadline: Deadline) -> EndedEdit {
        let Ok(instant) = crate::tree::locator_deadline::from_operation(deadline) else {
            return EndedEdit::NotAttempted;
        };
        if crate::tree::copy_bool_attr(field, kAXFocusedAttribute, deadline) != Some(true) {
            return EndedEdit::NotAttempted;
        }
        let ancestors = ancestors_to_window(field, instant);
        let Some(peer) = nearest_peer(
            &ancestors,
            |node| crate::tree::same_element(node, field),
            |node| surface_read::elements(node, "AXChildren", instant).unwrap_or_default(),
            |node| is_focus_peer(node, deadline),
        ) else {
            return EndedEdit::NotAttempted;
        };
        let neighbours = ancestors.first().map_or_else(Vec::new, |parent| {
            neighbour_values(parent, field, instant, deadline)
        });
        if let Err(error) = ax_helpers::ax_focus_or_err(&peer, deadline)
            && error.disposition == agent_desktop_core::DeliverySemantics::not_delivered()
        {
            return EndedEdit::NotAttempted;
        }
        if crate::tree::copy_bool_attr(&peer, kAXFocusedAttribute, deadline) != Some(true) {
            return EndedEdit::Refused;
        }
        let returned = ax_helpers::ax_focus_or_err(field, deadline).is_ok()
            && crate::tree::copy_bool_attr(field, kAXFocusedAttribute, deadline) == Some(true);
        let after: Vec<Option<String>> = neighbours
            .iter()
            .map(|(element, _, _)| crate::tree::copy_value_typed(element, deadline))
            .collect();
        let before: Vec<(String, Option<String>)> = neighbours
            .into_iter()
            .map(|(_, role, value)| (role, value))
            .collect();
        EndedEdit::Committed(json!({
            "mechanism": "focus_round_trip",
            "focus_returned": returned,
            "dependent_changes": changed_values(&before, &after),
        }))
    }

    fn ancestors_to_window(field: &AXElement, instant: std::time::Instant) -> Vec<AXElement> {
        let mut chain = Vec::new();
        let mut current = surface_read::element(field, "AXParent", instant)
            .ok()
            .flatten();
        while let Some(element) = current {
            let role = surface_read::string(&element, "AXRole", instant)
                .ok()
                .flatten();
            if matches!(role.as_deref(), Some("AXApplication") | None) {
                break;
            }
            let is_window = role.as_deref() == Some("AXWindow");
            current = (!is_window)
                .then(|| {
                    surface_read::element(&element, "AXParent", instant)
                        .ok()
                        .flatten()
                })
                .flatten();
            chain.push(element);
            if is_window || chain.len() > 32 {
                break;
            }
        }
        chain
    }

    fn is_focus_peer(element: &AXElement, deadline: Deadline) -> bool {
        let read = |attribute: &str| {
            let read_deadline =
                crate::tree::properties::ax_ipc::AxDeadline::absolute(deadline).ok()?;
            crate::tree::properties::attributes::copy_string_attr_result(
                element,
                attribute,
                read_deadline,
            )
            .ok()
            .flatten()
        };
        let enabled = crate::tree::copy_bool_attr(element, "AXEnabled", deadline);
        takes_focus_without_acting(
            read("AXRole").as_deref(),
            read("AXSubrole").as_deref(),
            enabled,
        ) && ax_helpers::is_attr_settable(element, kAXFocusedAttribute, deadline).unwrap_or(false)
    }

    fn neighbour_values(
        parent: &AXElement,
        field: &AXElement,
        instant: std::time::Instant,
        deadline: Deadline,
    ) -> Vec<(AXElement, String, Option<String>)> {
        let mut values = Vec::new();
        let mut pending = std::collections::VecDeque::from(
            surface_read::elements(parent, "AXChildren", instant).unwrap_or_default(),
        );
        while let Some(element) = pending.pop_front() {
            if values.len() >= MAX_NEIGHBOUR_VALUES {
                break;
            }
            if crate::tree::same_element(&element, field) {
                continue;
            }
            let role = surface_read::string(&element, "AXRole", instant)
                .ok()
                .flatten();
            if let Some(role) = role {
                let value = crate::tree::copy_value_typed(&element, deadline);
                if value.is_some() {
                    values.push((
                        element.clone(),
                        crate::tree::roles::ax_role_to_str(&role).to_string(),
                        value,
                    ));
                }
            }
            pending.extend(
                surface_read::elements(&element, "AXChildren", instant).unwrap_or_default(),
            );
        }
        values
    }
}

#[cfg(test)]
#[path = "edit_end_tests.rs"]
mod tests;
