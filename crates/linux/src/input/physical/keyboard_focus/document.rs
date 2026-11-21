use std::collections::BTreeMap;

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode, NativeHandle};
use futures_util::future::join;
use futures_util::stream::{self, StreamExt};
use serde_json::json;

use super::{EDITABLE_TEXT, bus_error, internal_editor_focused, role_number};
use crate::actions::write::mutation::{Fault, fault_of};
use crate::system::{ACCESSIBLE_INTERFACE, SharedBus, within};
use crate::tree::capabilities::action_read::read_type_target;
use crate::tree::element::ancestry;
use crate::tree::vocabulary::role_numbers::is_text_document;
use crate::tree::vocabulary::roles::role_of;
use crate::tree::vocabulary::state_bits::{EDITABLE, FOCUSED, READ_ONLY, StateBits};
use crate::tree::{
    AtspiElement, ChildAddress, Field, WalkLimits, atspi_element, call, list_children,
};

const TEXT_INTERFACE: &str = "org.a11y.atspi.Text";
const DESCENDANTS_IN_FLIGHT: usize = 32;

pub(crate) struct InputTarget {
    pub(crate) handle: NativeHandle,
    pub(crate) details: serde_json::Value,
}

impl InputTarget {
    fn new(element: AtspiElement) -> Self {
        Self {
            details: json!({"role": element.resolved_role(), "path": element.path()}),
            handle: element.into_native_handle(),
        }
    }
}

struct PendingDescendant {
    element: AtspiElement,
    depth: usize,
    ancestors: Vec<String>,
}

struct DescendantStep {
    candidate: Option<AtspiElement>,
    children: Vec<ChildAddress>,
    role: Option<u32>,
}

fn document_refused() -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionNotSupported,
        "The editable document has no uniquely focused writable text descendant",
    )
    .with_suggestion(
        "Focus the intended text inside the document and retry, or target its editable paragraph directly",
    )
    .with_disposition(DeliverySemantics::not_delivered())
}

fn document_bus_error(error: zbus::Error) -> AdapterError {
    if fault_of(&error) == Fault::TimedOut {
        bus_error(error)
    } else {
        document_refused()
    }
}

/// A text document without its own writable text interface routes insertion to its
/// uniquely focused writable `Text` + `EditableText` descendant found by the
/// generalized internal-editor search.
pub(crate) fn document_input_target(
    bus: &SharedBus,
    handle: &NativeHandle,
    deadline: Deadline,
) -> Result<Option<InputTarget>, AdapterError> {
    let view = read_type_target(bus, handle, deadline)?;
    let Some(role) = view.role() else {
        return Err(document_refused());
    };
    if !is_text_document(role) {
        return Ok(None);
    }
    if view.state(EDITABLE) != Some(true) || view.state(READ_ONLY) != Some(false) {
        return Err(document_refused());
    }
    let interfaces = view.interfaces().ok_or_else(document_refused)?;
    if has_text_interfaces(interfaces) {
        return Ok(None);
    }
    let root = atspi_element(handle)?;
    let connection = bus.get(deadline)?;
    let editor = within(
        deadline,
        internal_editor_focused(connection.inner(), root, Some(role)),
    )
    .map_err(|error| error.with_disposition(DeliverySemantics::not_delivered()))?
    .ok_or_else(document_refused)?;
    Ok(Some(InputTarget::new(editor)))
}

fn has_text_interfaces(interfaces: &[String]) -> bool {
    [TEXT_INTERFACE, EDITABLE_TEXT]
        .iter()
        .all(|needed| interfaces.iter().any(|name| name == needed))
}

async fn attributes_of(
    connection: &zbus::Connection,
    element: &AtspiElement,
) -> Result<Field<BTreeMap<String, String>>, AdapterError> {
    match call(
        connection,
        element,
        ACCESSIBLE_INTERFACE,
        "GetAttributes",
        &(),
    )
    .await
    {
        Ok(reply) => Ok(reply
            .body()
            .deserialize::<BTreeMap<String, String>>()
            .map(Field::Present)
            .unwrap_or(Field::Absent)),
        Err(error) if fault_of(&error) == Fault::TimedOut => Err(bus_error(error)),
        Err(_) => Ok(Field::Absent),
    }
}

async fn read_descendant(
    connection: &zbus::Connection,
    element: &AtspiElement,
) -> Result<Option<DescendantStep>, AdapterError> {
    let (state_reply, child_reply) = join(
        call(connection, element, ACCESSIBLE_INTERFACE, "GetState", &()),
        list_children(connection, element),
    )
    .await;
    let words: Vec<u32> = state_reply
        .map_err(document_bus_error)?
        .body()
        .deserialize()
        .map_err(|_| document_refused())?;
    let bits = StateBits::new(&words);
    if bits.has(READ_ONLY) {
        return Ok(None);
    }
    let children = child_reply?;
    let mut role = None;
    let mut candidate = None;
    if bits.has(EDITABLE) && bits.has(FOCUSED) {
        let interfaces: Vec<String> = call(
            connection,
            element,
            ACCESSIBLE_INTERFACE,
            "GetInterfaces",
            &(),
        )
        .await
        .map_err(document_bus_error)?
        .body()
        .deserialize()
        .map_err(|_| document_refused())?;
        if has_text_interfaces(&interfaces) {
            let (found_role, attrs) = join(
                role_number(connection, element),
                attributes_of(connection, element),
            )
            .await;
            let number = found_role?.ok_or_else(document_refused)?;
            role = Some(number);
            let attrs = attrs?;
            let resolved = role_of(
                number,
                &Field::Present(words),
                &Field::Present(interfaces),
                &attrs,
            )
            .as_str()
            .to_owned();
            let mut matched = element.clone();
            matched.set_resolved_role(resolved);
            candidate = Some(matched);
        }
    }
    if role.is_none() && !children.is_empty() {
        role = role_number(connection, element).await?;
    }
    Ok(Some(DescendantStep {
        candidate,
        children,
        role,
    }))
}

pub(super) async fn focused_document_descendant(
    connection: &zbus::Connection,
    root: &AtspiElement,
    root_role: u32,
) -> Result<AtspiElement, AdapterError> {
    let limits = WalkLimits::default();
    let owner = root.bus_name();
    let root_children = list_children(connection, root).await?;
    if root_children.len() > limits.max_children {
        return Err(document_refused());
    }
    let root_ancestry = ancestry::child(root, Some(root_role));
    let mut frontier: Vec<PendingDescendant> = root_children
        .into_iter()
        .filter(|(bus, path)| bus == owner && path != root.path())
        .map(|(bus, path)| {
            let mut child = AtspiElement::new(bus, path.as_str(), root.pid());
            child.set_ancestry(root_ancestry.clone());
            PendingDescendant {
                element: child,
                depth: 1,
                ancestors: vec![root.path().to_owned(), path],
            }
        })
        .collect();
    let mut visited: usize = 1;
    let mut found = None;
    while !frontier.is_empty() {
        visited = visited.saturating_add(frontier.len());
        if visited > limits.max_nodes {
            return Err(document_refused());
        }
        let steps: Vec<Result<Option<DescendantStep>, AdapterError>> = stream::iter(
            frontier
                .iter()
                .map(|pending| read_descendant(connection, &pending.element)),
        )
        .buffered(DESCENDANTS_IN_FLIGHT)
        .collect()
        .await;
        let mut next = Vec::new();
        for (pending, step) in frontier.into_iter().zip(steps) {
            let Some(step) = step? else {
                continue;
            };
            if let Some(candidate) = step.candidate {
                if found
                    .as_ref()
                    .is_some_and(|previous| previous != &candidate)
                {
                    return Err(document_refused());
                }
                found = Some(candidate);
            }
            if step.children.is_empty() {
                continue;
            }
            if pending.depth >= limits.max_raw_depth || step.children.len() > limits.max_children {
                return Err(document_refused());
            }
            let child_ancestry = ancestry::child(&pending.element, step.role);
            for (bus, path) in step.children {
                if bus != owner || pending.ancestors.iter().any(|seen| seen == &path) {
                    continue;
                }
                if visited + next.len() >= limits.max_nodes {
                    return Err(document_refused());
                }
                let mut child = AtspiElement::new(bus, path.as_str(), root.pid());
                child.set_ancestry(child_ancestry.clone());
                let mut ancestors = pending.ancestors.clone();
                ancestors.push(path);
                next.push(PendingDescendant {
                    element: child,
                    depth: pending.depth + 1,
                    ancestors,
                });
            }
        }
        frontier = next;
    }
    found.ok_or_else(document_refused)
}

#[cfg(test)]
#[path = "document_tests.rs"]
mod tests;
