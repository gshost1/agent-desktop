//! The one read `clear` takes before it writes.
//!
//! An element that already holds no text is the state `clear` asks for, so
//! nothing is written and the result is `not_delivered`, safe to repeat, with
//! no commit hint. The same batched read carries the role and subrole that
//! judge whether the application has taken a write, so a write needs no
//! second role read afterwards. The count is `AXNumberOfCharacters`, which
//! answers without copying the text; a secure field hides its text from it,
//! and an element without the count is no proof of an empty field, so both
//! are written as before.

use agent_desktop_core::{Action, ActionResult, ActionStep, AdapterError, Deadline, StepMechanism};

use crate::actions::edit_commit::{self, EditCommit};

const PROBE_ATTRIBUTES: [&str; 3] = ["AXRole", "AXSubrole", "AXNumberOfCharacters"];

/// What the element showed before `clear` wrote it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ClearProbe {
    pub(crate) role: Option<String>,
    pub(crate) subrole: Option<String>,
    pub(crate) characters: Option<i64>,
}

impl ClearProbe {
    fn already_clear(&self) -> bool {
        self.characters == Some(0) && self.subrole.as_deref() != Some("AXSecureTextField")
    }

    fn commit(&self) -> Option<EditCommit> {
        let role = self.role.as_deref()?;
        Some(EditCommit::from_roles(Some(role), self.subrole.as_deref()))
    }
}

/// Clears an element unless the probe shows it already empty, judging a
/// write's commit by the probe's roles, or by `read_commit` when the probe
/// could not read them.
pub(crate) fn clear_with(
    action: &Action,
    probe: &ClearProbe,
    write: impl FnOnce() -> Result<Vec<ActionStep>, AdapterError>,
    read_commit: impl FnOnce() -> EditCommit,
) -> Result<ActionResult, AdapterError> {
    if probe.already_clear() {
        return Ok(ActionResult::from_execution(
            action,
            vec![already_clear_step()],
        ));
    }
    let steps = write()?;
    let commit = probe.commit();
    Ok(edit_commit::value_write_result(action, steps, || {
        commit.unwrap_or_else(read_commit)
    }))
}

fn already_clear_step() -> ActionStep {
    ActionStep::skipped("AlreadyEmpty")
        .with_mechanism(StepMechanism::SemanticApi)
        .with_verified(true)
}

/// Reads the probe in one IPC round trip. A failed or partial read leaves
/// the unread fields `None`, which proves nothing and lets the write run.
#[cfg(target_os = "macos")]
pub(crate) fn read_clear_probe(element: &crate::tree::AXElement, deadline: Deadline) -> ClearProbe {
    use core_foundation::{
        array::CFArray,
        base::{CFType, TCFType},
        number::CFNumber,
        string::CFString,
    };
    let names = CFArray::from_CFTypes(&PROBE_ATTRIBUTES.map(CFString::new));
    let (error, values) = crate::tree::properties::ax_ipc::copy_multiple_attribute_values(
        element,
        names.as_concrete_TypeRef(),
        deadline,
    );
    let values = crate::tree::properties::cf_type::created_cf_array(values);
    if error != accessibility_sys::kAXErrorSuccess {
        return ClearProbe::default();
    }
    let Some(values) = values.filter(|values| values.len() as usize == PROBE_ATTRIBUTES.len())
    else {
        return ClearProbe::default();
    };
    let slot = |index: usize| -> Option<CFType> {
        use crate::tree::properties::node_attribute_decode::{is_null, slot_error};
        let item = values.get(index as isize)?;
        (!is_null(&item) && slot_error(&item).is_none()).then(|| item.clone())
    };
    let text = |index: usize| {
        slot(index)?
            .downcast::<CFString>()
            .map(|text| text.to_string())
    };
    ClearProbe {
        role: text(0),
        subrole: text(1),
        characters: slot(2)
            .and_then(|item| item.downcast::<CFNumber>())
            .and_then(|count| count.to_i64()),
    }
}

#[cfg(test)]
#[path = "clear_probe_tests.rs"]
mod tests;
