//! Ends the edit a value write left open in a single-line text field, so the
//! application takes the value now rather than whenever focus next moves.
//!
//! An AppKit field hands its text to the application's model only when
//! editing ends. Neither `AXConfirm` nor clearing `AXFocused` ends it: the
//! first is the field's own action, which Return also sends and which can
//! submit a form, and the second is refused. Focus moving to another element
//! does end it, so focus goes to the nearest other plain text field in the
//! same window, which takes focus without acting, and then back to the
//! written field. Only an enabled text field with no subrole whose focus is
//! settable qualifies: a search field can act on focus, a secure field
//! changes secure input, and buttons, menus and every other control are
//! never focused. Focus must be seen leaving the field and coming back. A
//! window with no such field keeps the edit open and the result says how to
//! end it. Values of the field's neighbours, read on both sides of the
//! commit, show what it changed; secure fields are never read.

use agent_desktop_core::{
    Action, ActionResult, ActionStep, AdapterError, Deadline, DeliverySemantics, ErrorCode,
    StepMechanism,
};
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
    /// Focus left the field, so the app has had the value, but it did not
    /// come back.
    NotRestored(Value),
    /// No edit to end, or no other field to move focus to.
    NotAttempted,
}

const REFUSED_SUGGESTION: &str = "The app would not end the edit, which it does when it rejects the value; it may be showing an alert. Run `snapshot --app <App> --surface alert -i`, answer the alert (a Discard or Revert button restores the old value), then write a value the field accepts, in its range and format.";

const NOT_RESTORED_SUGGESTION: &str = "Editing ended, so the app has had the value, but focus stayed on another field. Read the field back with `get <ref> --property value`, then run `focus <ref>` before you type or press keys.";

/// The result of a `set-value` on a text field: committed when the edit
/// could be ended and focus came back, refused when the app would not let it
/// end, otherwise marked as waiting for the commit.
#[cfg(target_os = "macos")]
pub(crate) fn set_value_result(
    action: &Action,
    field: &crate::tree::AXElement,
    mut steps: Vec<ActionStep>,
    deadline: Deadline,
) -> Result<ActionResult, AdapterError> {
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
        EndedEdit::NotRestored(evidence) => Err(focus_not_restored_error(evidence)),
        EndedEdit::NotAttempted => Ok(edit_commit::value_write_result(action, steps, || commit)),
    }
}

pub(crate) fn refused_commit_error() -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "The value was written, but the app refused to end the edit, so it did not take the value",
    )
    .with_suggestion(REFUSED_SUGGESTION)
    .with_details(json!({ "kind": "edit_commit_refused", "application_commit": "refused" }))
    .with_disposition(DeliverySemantics::delivered_unverified())
}

pub(crate) fn focus_not_restored_error(evidence: Value) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "The edit was ended, but focus did not return to the field",
    )
    .with_suggestion(NOT_RESTORED_SUGGESTION)
    .with_details(json!({
        "kind": "edit_focus_not_restored",
        "application_commit": "not_verified",
        "commit_evidence": evidence,
    }))
    .with_disposition(DeliverySemantics::delivered_unverified())
}

/// Whether focusing an element ends another field's edit without acting: an
/// enabled text field with no subrole whose focus is settable. `subrole` is
/// `None` when it could not be read and `Some(None)` when the field has none;
/// every fact must have been read.
pub(crate) fn takes_focus_without_acting(
    role: Option<&str>,
    subrole: Option<Option<&str>>,
    enabled: Option<bool>,
    focus_settable: Option<bool>,
) -> bool {
    role == Some("AXTextField")
        && subrole == Some(None)
        && enabled == Some(true)
        && focus_settable == Some(true)
}

/// Settles a focus round trip from what was read after each move: the peer
/// took focus and the field lost it, then the field took it back.
pub(crate) fn settle_round_trip(
    peer_took_focus: Option<bool>,
    field_kept_focus: Option<bool>,
    field_took_focus_back: Option<bool>,
    evidence: Value,
) -> EndedEdit {
    if peer_took_focus != Some(true) || field_kept_focus != Some(false) {
        return EndedEdit::Refused;
    }
    if field_took_focus_back == Some(true) {
        EndedEdit::Committed(evidence)
    } else {
        EndedEdit::NotRestored(evidence)
    }
}

/// Whether a neighbour's value may be read as evidence: never a secure
/// field, nor one whose subrole could not be read.
pub(crate) fn reports_value(role: &str, subrole: Option<Option<&str>>) -> bool {
    role != "AXSecureTextField"
        && matches!(subrole, Some(subrole) if subrole != Some("AXSecureTextField"))
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

/// Neighbour values read on both sides of the commit that differ, at most
/// [`MAX_REPORTED_CHANGES`] of them. A value unread after the commit is not
/// a change.
pub(crate) fn changed_values(before: &[(String, String)], after: &[Option<String>]) -> Vec<Value> {
    before
        .iter()
        .zip(after)
        .filter_map(|((role, old), new)| {
            new.as_ref()
                .filter(|new| *new != old)
                .map(|new| json!({ "role": role, "before": old, "after": new }))
        })
        .take(MAX_REPORTED_CHANGES)
        .collect()
}

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use crate::actions::ax_helpers;
    use crate::tree::{AXElement, surface_read};
    use accessibility_sys::kAXFocusedAttribute;

    type Neighbour = (AXElement, String, String);

    pub(super) fn end_edit(field: &AXElement, deadline: Deadline) -> EndedEdit {
        let Ok(instant) = crate::tree::locator_deadline::from_operation(deadline) else {
            return EndedEdit::NotAttempted;
        };
        if focused(field, deadline) != Some(true) {
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
            && error.disposition == DeliverySemantics::not_delivered()
        {
            return EndedEdit::NotAttempted;
        }
        let peer_took_focus = focused(&peer, deadline);
        let field_kept_focus = focused(field, deadline);
        if peer_took_focus != Some(true) || field_kept_focus != Some(false) {
            return settle_round_trip(peer_took_focus, field_kept_focus, None, Value::Null);
        }
        let field_took_focus_back = match ax_helpers::ax_focus_or_err(field, deadline) {
            Ok(_) => focused(field, deadline),
            Err(_) => None,
        };
        let after: Vec<Option<String>> = neighbours
            .iter()
            .map(|(element, _, _)| crate::tree::copy_value_typed(element, deadline))
            .collect();
        let before: Vec<(String, String)> = neighbours
            .into_iter()
            .map(|(_, role, value)| (role, value))
            .collect();
        let evidence = json!({
            "mechanism": "focus_round_trip",
            "focus_returned": field_took_focus_back == Some(true),
            "dependent_changes": changed_values(&before, &after),
        });
        settle_round_trip(
            peer_took_focus,
            field_kept_focus,
            field_took_focus_back,
            evidence,
        )
    }

    fn focused(element: &AXElement, deadline: Deadline) -> Option<bool> {
        let read_deadline = crate::tree::properties::ax_ipc::AxDeadline::absolute(deadline).ok()?;
        crate::tree::properties::attributes::copy_bool_attr_result(
            element,
            kAXFocusedAttribute,
            read_deadline,
        )
        .ok()
        .flatten()
    }

    fn string_read(
        element: &AXElement,
        attribute: &str,
        deadline: Deadline,
    ) -> Option<Option<String>> {
        let read_deadline = crate::tree::properties::ax_ipc::AxDeadline::absolute(deadline).ok()?;
        crate::tree::properties::attributes::copy_string_attr_result(
            element,
            attribute,
            read_deadline,
        )
        .ok()
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
        let role = string_read(element, "AXRole", deadline).flatten();
        if role.as_deref() != Some("AXTextField") {
            return false;
        }
        let subrole = string_read(element, "AXSubrole", deadline);
        let enabled = crate::tree::properties::ax_ipc::AxDeadline::absolute(deadline)
            .ok()
            .and_then(|read_deadline| {
                crate::tree::properties::attributes::copy_bool_attr_result(
                    element,
                    "AXEnabled",
                    read_deadline,
                )
                .ok()
                .flatten()
            });
        let focus_settable =
            ax_helpers::is_attr_settable(element, kAXFocusedAttribute, deadline).ok();
        takes_focus_without_acting(
            role.as_deref(),
            subrole.as_ref().map(|subrole| subrole.as_deref()),
            enabled,
            focus_settable,
        )
    }

    fn neighbour_values(
        parent: &AXElement,
        field: &AXElement,
        instant: std::time::Instant,
        deadline: Deadline,
    ) -> Vec<Neighbour> {
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
            if let Some(Some(role)) = string_read(&element, "AXRole", deadline) {
                let subrole = string_read(&element, "AXSubrole", deadline);
                if reports_value(&role, subrole.as_ref().map(|subrole| subrole.as_deref()))
                    && let Some(value) = crate::tree::copy_value_typed(&element, deadline)
                {
                    let role = crate::tree::roles::ax_role_to_str(&role).to_string();
                    values.push((element.clone(), role, value));
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
