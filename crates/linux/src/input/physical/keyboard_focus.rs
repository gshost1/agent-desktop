use std::time::Duration;

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};
use serde_json::json;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

use crate::actions::write::mutation::{Fault, fault_of};
use crate::input::deliver::ensure_budget;
use crate::input::physical::target_world::PhysicalWorld;
use crate::system::{ACCESSIBLE_INTERFACE, PROPERTIES_INTERFACE, SharedBus, within};
use crate::tree::vocabulary::role_numbers::is_text_document;
use crate::tree::vocabulary::state_bits::{EDITABLE, FOCUSED, READ_ONLY, StateBits};
use crate::tree::{AtspiElement, COMPONENT_INTERFACE, call};

const EDITABLE_TEXT: &str = "org.a11y.atspi.EditableText";

pub(crate) mod document;

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

fn bus_error(error: zbus::Error) -> AdapterError {
    if fault_of(&error) == Fault::TimedOut {
        AdapterError::timeout("The target focus request did not answer in time")
            .with_platform_detail(error.to_string())
            .with_disposition(DeliverySemantics::not_delivered())
    } else {
        refused()
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
    match internal_editor_focused(connection, element, None).await {
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
    role: Option<u32>,
) -> Result<Option<AtspiElement>, AdapterError> {
    let role = match (role, element.resolved_role()) {
        (Some(role), _) => Some(role),
        (None, Some("document") | None) => role_number(connection, element).await?,
        _ => None,
    };
    if let Some(role) = role.filter(|role| is_text_document(*role)) {
        return document::focused_document_descendant(connection, element, role)
            .await
            .map(Some);
    }
    hit_internal_editor(connection, element).await
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
