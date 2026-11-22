//! Ends an opted-in edit without focusing a neighbouring control.

use agent_desktop_core::{
    Action, ActionResult, ActionStep, AdapterError, Deadline, DeliverySemantics, ErrorCode,
    StepMechanism,
};
use serde_json::{Value, json};

const MAX_PEER_NODES: usize = 400;
const MAX_NEIGHBOUR_VALUES: usize = 64;
const MAX_REPORTED_CHANGES: usize = 3;

pub(crate) enum EndedEdit {
    Committed(Value),
    Refused,
    NotRestored(Value),
    NotAttempted,
    NoSafePeer(Value),
    Delegated,
}

const REFUSED_SUGGESTION: &str = "The app would not end the edit, which it does when it rejects the value; it may be showing an alert. Run `snapshot --app <App> --surface alert -i`, answer the alert (a Discard or Revert button restores the old value), then write a value the field accepts, in its range and format.";
const NOT_RESTORED_SUGGESTION: &str = "Editing ended, so the app has had the value, but focus did not return to the field. Read the field back with `get <ref> --property value`, then run `focus <ref>` before you type or press keys.";

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
                ActionStep::succeeded("EndEditing")
                    .with_mechanism(StepMechanism::SemanticApi)
                    .with_verified(true),
            );
            Ok(ActionResult::from_execution(action, steps).with_commit_ended(evidence))
        }
        EndedEdit::Refused => Err(refused_commit_error()),
        EndedEdit::NotRestored(evidence) => Err(focus_not_restored_error(evidence)),
        EndedEdit::NoSafePeer(details) => Err(no_safe_peer_error(details)),
        EndedEdit::Delegated => Err(focus_delegated_error()),
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

pub(crate) fn no_safe_peer_error(mut details: Value) -> AdapterError {
    details["kind"] = json!("edit_commit_no_safe_peer");
    details["application_commit"] = json!("unobserved");
    AdapterError::new(ErrorCode::ActionFailed,
        "The value was written, but the app showed no sign of taking it and no control could end the edit safely")
        .with_details(details)
        .with_disposition(DeliverySemantics::delivered_unverified())
        .with_suggestion("Run `focus` on another field you know does not act, then check the result. The value is written, but the app showed no sign of taking it. Do not press Return.")
}

pub(crate) fn focus_delegated_error() -> AdapterError {
    AdapterError::new(ErrorCode::ActionFailed,
        "The value was written, but the app delegated focus to a control")
        .with_details(json!({"kind": "edit_commit_focus_delegated", "application_commit": "not_verified"}))
        .with_disposition(DeliverySemantics::delivered_unverified())
        .with_suggestion("Read the value back with `get <ref> --property value` and check the app's result before acting again.")
}

pub(crate) fn safe_text_area(actions: Option<&[String]>) -> bool {
    actions.is_some_and(|actions| actions.len() == 1 && actions[0] == "AXShowMenu")
}

pub(crate) fn safe_focused_role(role: Option<&str>, actions: Option<&[String]>) -> bool {
    match role {
        Some("AXWindow" | "AXGroup" | "AXScrollArea" | "AXStaticText") => true,
        Some("AXTextArea") => safe_text_area(actions),
        _ => false,
    }
}

pub(crate) fn safe_subtree_node(role: Option<&str>, focus_settable: Option<bool>) -> bool {
    matches!(
        role,
        Some("AXGroup" | "AXScrollArea" | "AXStaticText" | "AXImage" | "AXTextArea")
    ) || focus_settable == Some(false)
}

pub(crate) fn confirm_evidence(field_focused: Option<bool>, changed: bool) -> bool {
    field_focused == Some(false) || changed
}

pub(crate) enum Trial {
    Continue,
    Ended(Vec<Value>),
    Delegated,
    Refused,
}

pub(crate) fn ordered_attempts<T>(
    candidates: impl IntoIterator<Item = (&'static str, T)>,
    mut attempt: impl FnMut(&str, T) -> Trial,
) -> (EndedEdit, Option<&'static str>) {
    let mut attempted = Vec::new();
    let mut confirm_sent = false;
    for (mechanism, target) in candidates {
        if !attempted.contains(&mechanism) {
            attempted.push(mechanism);
        }
        confirm_sent |= mechanism == "confirm";
        match attempt(mechanism, target) {
            Trial::Continue => {}
            Trial::Ended(changes) => {
                return (
                    EndedEdit::Committed(json!({
                        "mechanism": mechanism, "dependent_changes": changes,
                    })),
                    Some(mechanism),
                );
            }
            Trial::Delegated => return (EndedEdit::Delegated, Some(mechanism)),
            Trial::Refused => return (EndedEdit::Refused, Some(mechanism)),
        }
    }
    (
        EndedEdit::NoSafePeer(json!({"attempted": attempted, "confirm_sent": confirm_sent})),
        None,
    )
}

pub(crate) fn reports_value(role: &str, subrole: Option<Option<&str>>) -> bool {
    role != "AXSecureTextField"
        && matches!(subrole, Some(subrole) if subrole != Some("AXSecureTextField"))
}

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
#[path = "edit_end_ax.rs"]
mod imp;

#[cfg(test)]
#[path = "edit_end_tests.rs"]
mod tests;
