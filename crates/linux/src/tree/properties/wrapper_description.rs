use agent_desktop_core::{AdapterError, tree::roles::is_interactive_role};
use futures_util::future::join;
use zbus::Connection;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

use super::node_read::{call, deserialize, string_value};
use crate::system::{ACCESSIBLE_INTERFACE, PROPERTIES_INTERFACE};
use crate::tree::vocabulary::role_numbers::{FILLER, PANEL};
use crate::tree::vocabulary::roles::role_of;
use crate::tree::{AtspiElement, Field, RawNode};

fn optional<T>(result: Result<T, zbus::Error>) -> Result<Field<T>, AdapterError> {
    match result {
        Ok(value) => Ok(Field::Present(value)),
        Err(error) if crate::system::is_connection_failure(&error) => {
            Err(crate::system::classify(&error))
        }
        Err(_) => Ok(Field::Absent),
    }
}

async fn read_text(
    connection: &Connection,
    element: &AtspiElement,
    property: &str,
) -> Result<Field<String>, AdapterError> {
    let reply = call(
        connection,
        element,
        PROPERTIES_INTERFACE,
        "Get",
        &(ACCESSIBLE_INTERFACE, property),
    )
    .await;
    Ok(match optional(deserialize::<OwnedValue>(reply))? {
        Field::Present(value) => string_value(&value),
        _ => Field::Absent,
    })
}

fn needs_description(node: &RawNode) -> bool {
    node.properties.name.is_blank()
        && node.properties.description.is_blank()
        && node.traits.role.present().is_some_and(|number| {
            is_interactive_role(
                role_of(
                    *number,
                    &node.traits.states,
                    &node.traits.interfaces,
                    &node.attributes,
                )
                .as_str(),
            )
        })
}

async fn parent(
    connection: &Connection,
    element: &AtspiElement,
) -> Result<Option<AtspiElement>, zbus::Error> {
    let value = deserialize::<OwnedValue>(
        call(
            connection,
            element,
            PROPERTIES_INTERFACE,
            "Get",
            &(ACCESSIBLE_INTERFACE, "Parent"),
        )
        .await,
    )?;
    let (bus, path) = <(String, OwnedObjectPath)>::try_from(value)?;
    Ok(
        (bus == element.bus_name() && path.as_str() != "/org/a11y/atspi/null")
            .then(|| AtspiElement::new(bus, path.to_string(), element.pid())),
    )
}

async fn read_description(
    connection: &Connection,
    element: &AtspiElement,
) -> Result<Field<String>, AdapterError> {
    let parent = match optional(parent(connection, element).await)? {
        Field::Present(Some(parent)) => parent,
        _ => return Ok(Field::Absent),
    };
    let (role, children) = join(
        call(connection, &parent, ACCESSIBLE_INTERFACE, "GetRole", &()),
        call(
            connection,
            &parent,
            PROPERTIES_INTERFACE,
            "Get",
            &(ACCESSIBLE_INTERFACE, "ChildCount"),
        ),
    )
    .await;
    let role = optional(deserialize::<u32>(role))?;
    let children = optional(
        deserialize::<OwnedValue>(children).and_then(|value| value.try_into().map_err(Into::into)),
    )?;
    if !matches!(role.present(), Some(&PANEL | &FILLER)) || children.present() != Some(&1) {
        return Ok(Field::Absent);
    }
    let (name, description) = join(
        read_text(connection, &parent, "Name"),
        read_text(connection, &parent, "Description"),
    )
    .await;
    Ok(select_description(&role, &name?, description?, &children))
}

fn select_description(
    role: &Field<u32>,
    name: &Field<String>,
    description: Field<String>,
    children: &Field<i32>,
) -> Field<String> {
    if matches!(role.present(), Some(&PANEL | &FILLER))
        && name.present().is_some_and(|name| name.trim().is_empty())
        && children.present() == Some(&1)
    {
        return description;
    }
    Field::Absent
}

pub(in crate::tree) async fn read_wrapper_description(
    connection: &Connection,
    node: &RawNode,
) -> Result<Option<String>, AdapterError> {
    if !needs_description(node) {
        return Ok(None);
    }
    let description = read_description(connection, &node.element).await?;
    Ok(description.non_blank().cloned())
}

#[cfg(test)]
#[path = "wrapper_description_tests.rs"]
mod tests;
