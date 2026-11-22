//! Routes text writes aimed at a field to the inline editor that is editing
//! that field in place (see `crate::tree::inline_editor`).

use agent_desktop_core::{AdapterError, Deadline, actions::capability};

use crate::tree::AXElement;

/// The key that commits an in-place edit in the editors this routing proves.
pub(crate) const COMMIT_KEY: &str = "return";

const EDITABLE_ROLES: [&str; 2] = ["textfield", "combobox"];

pub(crate) fn refuse_commit(action: &agent_desktop_core::Action) -> Result<(), AdapterError> {
    if matches!(action, agent_desktop_core::Action::SetValueAndCommit(_)) {
        return Err(AdapterError::new(agent_desktop_core::ErrorCode::ActionNotSupported,
            "Ending an inline editor's edit safely is not supported")
            .with_details(serde_json::json!({"kind": "edit_commit_unsupported_target"}))
            .with_disposition(agent_desktop_core::DeliverySemantics::not_delivered())
            .with_suggestion("Drop --commit to write into the inline editor, then end that edit using the app's documented commit mechanism and verify the result."));
    }
    Ok(())
}

pub(crate) fn editor_for(
    field: &AXElement,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    crate::tree::inline_editor::live_editor_for(
        field,
        crate::tree::locator_deadline::from_operation(deadline)?,
    )
}

/// Live actionability of a field in an in-place edit: `type` and `set-value`
/// reach its editor even where the field itself accepts no inserted text.
pub(crate) fn with_editor_capabilities(
    field: &AXElement,
    role: &str,
    mut actions: Vec<String>,
    deadline: Deadline,
) -> Result<Vec<String>, AdapterError> {
    if needs_editor_probe(role, &actions) && editor_for(field, deadline)?.is_some() {
        add_editor_capabilities(&mut actions);
    }
    Ok(actions)
}

fn needs_editor_probe(role: &str, actions: &[String]) -> bool {
    EDITABLE_ROLES.contains(&role)
        && !(capability::contains(actions, capability::TYPE_TEXT)
            && capability::contains(actions, capability::SET_VALUE))
}

fn add_editor_capabilities(actions: &mut Vec<String>) {
    for granted in [capability::SET_VALUE, capability::TYPE_TEXT] {
        if !capability::contains(actions, granted) {
            actions.push(granted.to_string());
        }
    }
}

#[cfg(test)]
#[path = "inline_editor_target_tests.rs"]
mod tests;
