//! An inline editor is the text editor an application floats over a field
//! while it edits that field in place, as a list does when it renames a row.
//! The editor holds keyboard focus, belongs to the field's process, has no
//! window of its own and sits over the field. A value written to the field
//! underneath changes what the field reports but not what the editor commits,
//! so text writes go to the editor once every one of those facts is observed.

use agent_desktop_core::{AdapterError, Rect};
use std::time::Instant;

use super::AXElement;

const EDITED_ROLES: [&str; 3] = ["AXTextField", "AXTextArea", "AXComboBox"];
const EDITOR_ROLES: [&str; 2] = ["AXTextField", "AXTextArea"];
const MIN_SHARED_AREA: f64 = 0.5;

/// What one element reports about itself for the inline-editor proof.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EditorFacts {
    pub(crate) pid: i32,
    pub(crate) role: Option<String>,
    pub(crate) bounds: Option<Rect>,
    pub(crate) has_window: bool,
}

/// True only when every observed fact shows `editor` editing `field` in place.
/// A missing fact is no proof.
pub(crate) fn edits_in_place(field: &EditorFacts, editor: &EditorFacts) -> bool {
    field.pid == editor.pid
        && role_in(field.role.as_deref(), &EDITED_ROLES)
        && is_windowless_editor(editor)
        && field
            .bounds
            .zip(editor.bounds)
            .is_some_and(|(field, editor)| covers(field, editor))
}

pub(crate) fn is_windowless_editor(facts: &EditorFacts) -> bool {
    role_in(facts.role.as_deref(), &EDITOR_ROLES) && !facts.has_window
}

pub(crate) fn may_be_edited_in_place(ax_role: Option<&str>) -> bool {
    role_in(ax_role, &EDITED_ROLES)
}

fn role_in(role: Option<&str>, roles: &[&str]) -> bool {
    role.is_some_and(|role| roles.contains(&role))
}

fn covers(field: Rect, editor: Rect) -> bool {
    let width = (field.x + field.width).min(editor.x + editor.width) - field.x.max(editor.x);
    let height = (field.y + field.height).min(editor.y + editor.height) - field.y.max(editor.y);
    if width <= 0.0 || height <= 0.0 {
        return false;
    }
    let smaller = (field.width * field.height).min(editor.width * editor.height);
    smaller > 0.0 && width * height >= smaller * MIN_SHARED_AREA
}

/// The editor editing `field` in place, or `None` when the relationship
/// cannot be proven from the application's focused element.
#[cfg(target_os = "macos")]
pub(crate) fn live_editor_for(
    field: &AXElement,
    deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    let Some(field_facts) = read_facts(field, deadline)? else {
        return Ok(None);
    };
    if !may_be_edited_in_place(field_facts.role.as_deref()) {
        return Ok(None);
    }
    let Some(editor) = focused_element(field_facts.pid, deadline)? else {
        return Ok(None);
    };
    if super::same_element(&editor, field) {
        return Ok(None);
    }
    let Some(editor_facts) = read_facts(&editor, deadline)? else {
        return Ok(None);
    };
    Ok(edits_in_place(&field_facts, &editor_facts).then_some(editor))
}

/// The process's focused element when it is a windowless text editor.
#[cfg(target_os = "macos")]
pub(crate) fn focused_editor_for_pid(
    pid: i32,
    deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    let Some(focused) = focused_element(pid, deadline)? else {
        return Ok(None);
    };
    let Some(facts) = read_facts(&focused, deadline)? else {
        return Ok(None);
    };
    Ok((facts.pid == pid && is_windowless_editor(&facts)).then_some(focused))
}

#[cfg(target_os = "macos")]
fn focused_element(pid: i32, deadline: Instant) -> Result<Option<AXElement>, AdapterError> {
    let app = super::element_for_pid(pid);
    super::surface_read::element(&app, "AXFocusedUIElement", deadline)
}

#[cfg(target_os = "macos")]
fn read_facts(element: &AXElement, deadline: Instant) -> Result<Option<EditorFacts>, AdapterError> {
    let Ok(pid) = super::properties::ax_ipc::pid(element, deadline) else {
        return Ok(None);
    };
    let role = super::surface_read::string(element, "AXRole", deadline)?;
    let has_window = super::surface_read::element(element, "AXWindow", deadline)?.is_some();
    let bounds = super::element_bounds::read_bounds_with_deadline(element, deadline)?;
    Ok(Some(EditorFacts {
        pid,
        role,
        bounds,
        has_window,
    }))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn live_editor_for(
    _field: &AXElement,
    _deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    Ok(None)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn focused_editor_for_pid(
    _pid: i32,
    _deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    Ok(None)
}

#[cfg(test)]
#[path = "inline_editor_tests.rs"]
mod tests;
