use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode, NativeHandle};

use super::action_read::read_type_target;
use crate::system::SharedBus;
use crate::tree::properties::read_purpose::ReadPurpose;
use crate::tree::vocabulary::role_numbers::is_text_document;
use crate::tree::vocabulary::state_bits::{EDITABLE, FOCUSED, READ_ONLY, StateBits};
use crate::tree::walk::walker::walk_tree_for;
use crate::tree::{AtspiElement, ObservedRead, Walk, WalkLimits, WalkedNode, atspi_element};

const TEXT_INTERFACE: &str = "org.a11y.atspi.Text";
const EDITABLE_TEXT: &str = "org.a11y.atspi.EditableText";

pub(crate) struct InputTarget {
    pub(crate) handle: NativeHandle,
    pub(crate) details: serde_json::Value,
}

impl InputTarget {
    fn new(element: AtspiElement) -> Self {
        Self {
            details: serde_json::json!({"role": element.resolved_role(), "path": element.path()}),
            handle: element.into_native_handle(),
        }
    }
}

pub(crate) fn needs_routing(role: Option<&str>) -> bool {
    role == Some("document")
}

fn refused() -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionNotSupported,
        "The editable document has no uniquely focused writable text descendant",
    )
    .with_suggestion("Focus the intended text inside the document and retry, or target its editable paragraph directly")
    .with_disposition(DeliverySemantics::not_delivered())
}

fn collect(
    node: &WalkedNode<ObservedRead>,
    owner: &str,
    found: &mut Vec<AtspiElement>,
) -> Result<(), AdapterError> {
    let raw = &node.payload.node;
    if raw.element.bus_name() != owner {
        return Ok(());
    }
    let Some(words) = raw.traits.states.present() else {
        return Err(refused());
    };
    let bits = StateBits::new(words);
    if bits.has(READ_ONLY) {
        return Ok(());
    }
    if bits.has(EDITABLE) && bits.has(FOCUSED) {
        let interfaces = raw.traits.interfaces.present().ok_or_else(refused)?;
        if [TEXT_INTERFACE, EDITABLE_TEXT]
            .iter()
            .all(|needed| interfaces.iter().any(|name| name == needed))
        {
            let mut element = raw.element.clone();
            let role = raw.traits.role.present().map(|role| {
                crate::tree::vocabulary::roles::role_of(
                    *role,
                    &raw.traits.states,
                    &raw.traits.interfaces,
                    &raw.attributes,
                )
                .as_str()
                .to_owned()
            });
            element.set_resolved_role(role.unwrap_or_else(|| "unknown".into()));
            found.push(element);
        }
    }
    for child in &node.children {
        collect(child, owner, found)?;
    }
    Ok(())
}

fn focused_editor(walk: &Walk<ObservedRead>) -> Result<AtspiElement, AdapterError> {
    let raw = &walk.root.payload.node;
    let writable = raw.traits.states.present().is_some_and(|words| {
        let bits = StateBits::new(words);
        bits.has(EDITABLE) && !bits.has(READ_ONLY)
    });
    if !walk.complete
        || !writable
        || !raw
            .traits
            .role
            .present()
            .is_some_and(|role| is_text_document(*role))
    {
        return Err(refused());
    }
    let mut found = Vec::new();
    collect(&walk.root, raw.element.bus_name(), &mut found)?;
    if found.len() != 1 {
        return Err(refused());
    }
    found.pop().ok_or_else(refused)
}

/// A document without its own Text interface can insert only through its
/// uniquely focused, writable text descendant in a complete fresh walk.
pub(crate) fn input_target(
    bus: &SharedBus,
    handle: &NativeHandle,
    deadline: Deadline,
) -> Result<Option<InputTarget>, AdapterError> {
    let view = read_type_target(bus, handle, deadline)?;
    if !view.role().is_some_and(is_text_document)
        || view
            .interfaces()
            .is_some_and(|interfaces| interfaces.iter().any(|name| name == TEXT_INTERFACE))
    {
        return Ok(None);
    }
    if view.state(EDITABLE) != Some(true) || view.state(READ_ONLY) != Some(false) {
        return Err(refused());
    }
    let root = atspi_element(handle)?;
    let walk = walk_tree_for(
        bus,
        root,
        &WalkLimits::default(),
        deadline,
        ReadPurpose::Resolve,
    )?;
    focused_editor(&walk).map(|element| Some(InputTarget::new(element)))
}

#[cfg(test)]
#[path = "document_text_tests.rs"]
mod tests;
