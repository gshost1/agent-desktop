use std::collections::BTreeMap;
use std::time::Duration;

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode, NativeHandle};
use futures_util::future::join;
use futures_util::stream::{self, StreamExt};
use serde_json::json;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

use crate::actions::write::mutation::{Fault, fault_of};
use crate::input::deliver::ensure_budget;
use crate::input::physical::target_world::PhysicalWorld;
use crate::system::{ACCESSIBLE_INTERFACE, PROPERTIES_INTERFACE, SharedBus, within};
use crate::tree::capabilities::action_read::read_type_target;
use crate::tree::element::ancestry;
use crate::tree::vocabulary::role_numbers::is_text_document;
use crate::tree::vocabulary::roles::role_of;
use crate::tree::vocabulary::state_bits::{EDITABLE, FOCUSED, READ_ONLY, StateBits};
use crate::tree::{AtspiElement, COMPONENT_INTERFACE, Field, WalkLimits, atspi_element, call};

const TEXT_INTERFACE: &str = "org.a11y.atspi.Text";
const EDITABLE_TEXT: &str = "org.a11y.atspi.EditableText";
const NULL_PATH: &str = "/org/a11y/atspi/null";
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
    children: Vec<(String, String)>,
    role: Option<u32>,
}

pub(crate) fn needs_document_routing(role: Option<&str>) -> bool {
    role == Some("document")
}

pub(in crate::input) fn refused() -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "Keyboard focus on the target could not be proven; keyboard delivery was refused",
    )
    .with_details(json!({
        "check": "target_focus",
        "physical_delivery_started": false,
    }))
    .with_disposition(DeliverySemantics::not_delivered())
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

fn bus_error(error: zbus::Error) -> AdapterError {
    if fault_of(&error) == Fault::TimedOut {
        AdapterError::timeout("The target focus request did not answer in time")
            .with_platform_detail(error.to_string())
            .with_disposition(DeliverySemantics::not_delivered())
    } else {
        refused()
    }
}

fn document_bus_error(error: zbus::Error) -> AdapterError {
    if fault_of(&error) == Fault::TimedOut {
        bus_error(error)
    } else {
        document_refused()
    }
}

pub(crate) fn grab(
    bus: &SharedBus,
    element: &AtspiElement,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let connection = bus.get(deadline)?;
    within(deadline, async {
        connection
            .inner()
            .call_method(
                Some(element.bus_name()),
                element.path(),
                Some(COMPONENT_INTERFACE),
                "GrabFocus",
                &(),
            )
            .await
            .map_err(bus_error)?;
        Ok(())
    })
}

pub(crate) fn read(
    bus: &SharedBus,
    element: &AtspiElement,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let connection = bus.get(deadline)?;
    within(deadline, focused(connection.inner(), element))
}

/// A text document without its own `Text` interface routes insertion to its
/// uniquely focused writable `Text` + `EditableText` descendant found by the
/// generalized internal-editor search.
pub(crate) fn document_input_target(
    bus: &SharedBus,
    handle: &NativeHandle,
    deadline: Deadline,
) -> Result<Option<InputTarget>, AdapterError> {
    let view = read_type_target(bus, handle, deadline)?;
    let Some(role) = view.role().filter(|role| is_text_document(*role)) else {
        return Ok(None);
    };
    if view
        .interfaces()
        .is_some_and(|interfaces| interfaces.iter().any(|name| name == TEXT_INTERFACE))
    {
        return Ok(None);
    }
    if view.state(EDITABLE) != Some(true) || view.state(READ_ONLY) != Some(false) {
        return Err(document_refused());
    }
    let root = atspi_element(handle)?;
    let connection = bus.get(deadline)?;
    let editor = within(
        deadline,
        focused_document_descendant(connection.inner(), root, role),
    )?;
    Ok(Some(InputTarget::new(editor)))
}

async fn focused(
    connection: &zbus::Connection,
    element: &AtspiElement,
) -> Result<bool, AdapterError> {
    let words = states(connection, element).await?;
    let bits = StateBits::new(&words);
    if bits.has(FOCUSED) {
        return Ok(true);
    }
    if !bits.has(EDITABLE) || bits.has(READ_ONLY) {
        return Ok(false);
    }
    match internal_editor_focused(connection, element).await {
        Err(error) if error.code != ErrorCode::Timeout => Ok(false),
        Ok(editor) => Ok(editor.is_some()),
        Err(error) => Err(error),
    }
}

async fn states(
    connection: &zbus::Connection,
    element: &AtspiElement,
) -> Result<Vec<u32>, AdapterError> {
    call(connection, element, ACCESSIBLE_INTERFACE, "GetState", &())
        .await
        .map_err(bus_error)?
        .body()
        .deserialize()
        .map_err(|_| refused())
}

async fn role_number(
    connection: &zbus::Connection,
    element: &AtspiElement,
) -> Result<Option<u32>, AdapterError> {
    match call(connection, element, ACCESSIBLE_INTERFACE, "GetRole", &()).await {
        Ok(reply) => Ok(reply.body().deserialize::<u32>().ok()),
        Err(error) if fault_of(&error) == Fault::TimedOut => Err(bus_error(error)),
        Err(_) => Ok(None),
    }
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

async fn children_of(
    connection: &zbus::Connection,
    element: &AtspiElement,
) -> Result<Vec<(String, String)>, AdapterError> {
    let entries: Vec<(String, OwnedObjectPath)> = call(
        connection,
        element,
        ACCESSIBLE_INTERFACE,
        "GetChildren",
        &(),
    )
    .await
    .map_err(document_bus_error)?
    .body()
    .deserialize()
    .map_err(|_| document_refused())?;
    Ok(entries
        .into_iter()
        .filter(|(bus, path)| !bus.is_empty() && path.as_str() != NULL_PATH)
        .map(|(bus, path)| (bus, path.to_string()))
        .collect())
}

async fn parent(
    connection: &zbus::Connection,
    element: &AtspiElement,
) -> Result<AtspiElement, AdapterError> {
    let reply = call(
        connection,
        element,
        PROPERTIES_INTERFACE,
        "Get",
        &(ACCESSIBLE_INTERFACE, "Parent"),
    )
    .await
    .map_err(bus_error)?;
    let value: OwnedValue = reply.body().deserialize().map_err(|_| refused())?;
    let (bus, path) = <(String, OwnedObjectPath)>::try_from(value).map_err(|_| refused())?;
    Ok(AtspiElement::new(bus, path.to_string(), element.pid()))
}

async fn internal_editor_focused(
    connection: &zbus::Connection,
    element: &AtspiElement,
) -> Result<Option<AtspiElement>, AdapterError> {
    if let Some(role) = role_number(connection, element)
        .await?
        .filter(|role| is_text_document(*role))
    {
        return focused_document_descendant(connection, element, role)
            .await
            .map(Some);
    }
    hit_internal_editor(connection, element).await
}

async fn read_descendant(
    connection: &zbus::Connection,
    element: &AtspiElement,
) -> Result<Option<DescendantStep>, AdapterError> {
    let (state_reply, child_reply) = join(
        call(connection, element, ACCESSIBLE_INTERFACE, "GetState", &()),
        children_of(connection, element),
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
        if [TEXT_INTERFACE, EDITABLE_TEXT]
            .iter()
            .all(|needed| interfaces.iter().any(|name| name == needed))
        {
            let (found_role, attrs) = join(
                role_number(connection, element),
                attributes_of(connection, element),
            )
            .await;
            role = found_role?;
            let attrs = attrs?;
            let resolved = role
                .map(|number| {
                    role_of(
                        number,
                        &Field::Present(words),
                        &Field::Present(interfaces),
                        &attrs,
                    )
                    .as_str()
                    .to_owned()
                })
                .unwrap_or_else(|| "unknown".into());
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

async fn focused_document_descendant(
    connection: &zbus::Connection,
    root: &AtspiElement,
    root_role: u32,
) -> Result<AtspiElement, AdapterError> {
    let limits = WalkLimits::default();
    let owner = root.bus_name();
    let root_children = children_of(connection, root).await?;
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
    let mut found: Vec<AtspiElement> = Vec::new();
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
                found.push(candidate);
                if found.len() > 1 {
                    return Err(document_refused());
                }
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
    if found.len() != 1 {
        return Err(document_refused());
    }
    found.pop().ok_or_else(document_refused)
}

async fn hit_internal_editor(
    connection: &zbus::Connection,
    element: &AtspiElement,
) -> Result<Option<AtspiElement>, AdapterError> {
    const PARENT_COORDINATES: u32 = 2;
    if !crate::tree::properties::interface_support::supports(
        connection,
        element,
        COMPONENT_INTERFACE,
    )
    .await
    .map_err(bus_error)?
    {
        return Ok(None);
    }
    let extents = call(
        connection,
        element,
        COMPONENT_INTERFACE,
        "GetExtents",
        &(PARENT_COORDINATES,),
    )
    .await
    .map_err(bus_error)?;
    let (x, y, width, height): (i32, i32, i32, i32) =
        extents.body().deserialize().map_err(|_| refused())?;
    if width <= 0 || height <= 0 {
        return Ok(None);
    }
    let x = x.checked_add(width / 2).ok_or_else(refused)?;
    let y = y.checked_add(height / 2).ok_or_else(refused)?;
    let hit = call(
        connection,
        element,
        COMPONENT_INTERFACE,
        "GetAccessibleAtPoint",
        &(x, y, PARENT_COORDINATES),
    )
    .await
    .map_err(bus_error)?;
    let (bus, path): (String, OwnedObjectPath) = hit.body().deserialize().map_err(|_| refused())?;
    let mut candidate = AtspiElement::new(bus, path.to_string(), element.pid());
    for _ in 0..2 {
        if candidate.bus_name() != element.bus_name() || candidate == *element {
            return Ok(None);
        }
        let owner = parent(connection, &candidate).await?;
        if owner == *element {
            let interfaces = call(
                connection,
                &candidate,
                ACCESSIBLE_INTERFACE,
                "GetInterfaces",
                &(),
            )
            .await
            .map_err(bus_error)?;
            let interfaces: Vec<String> = interfaces.body().deserialize().map_err(|_| refused())?;
            let matched = interfaces.iter().any(|i| i == EDITABLE_TEXT)
                && StateBits::new(&states(connection, &candidate).await?).has(FOCUSED);
            return Ok(matched.then_some(candidate));
        }
        candidate = owner;
    }
    Ok(None)
}

pub(in crate::input) fn prepare(
    world: &impl PhysicalWorld,
    element: &AtspiElement,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    if world.target_focused(element, deadline)? {
        return Ok(());
    }
    world.focus_target(element, deadline)?;
    for attempt in 0..=10 {
        ensure_budget(deadline)?;
        if world.target_focused(element, deadline)? {
            return Ok(());
        }
        if attempt < 10 {
            world.pause(Duration::from_millis(50));
        }
    }
    Err(refused())
}

#[cfg(test)]
#[path = "keyboard_focus_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "keyboard_focus_document_tests.rs"]
mod document_tests;
