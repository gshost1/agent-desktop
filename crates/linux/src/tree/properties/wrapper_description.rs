use agent_desktop_core::{AdapterError, tree::roles::is_interactive_role};
use futures_util::future::join;
use zbus::Connection;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

use super::liveness::Liveness;
use super::node_read::{call, deserialize, get_one, string_value};
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
        Err(error) => Ok(Field::Failed(error.to_string())),
    }
}

async fn read_text(
    connection: &Connection,
    element: &AtspiElement,
    property: &'static str,
    liveness: Liveness,
) -> Result<Field<String>, AdapterError> {
    get_one(connection, element, property, string_value, liveness).await
}

fn needs_description(node: &RawNode) -> bool {
    !node.element.in_web_document()
        && node.properties.name.is_blank()
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
        Field::Failed(detail) => return Ok(Field::Failed(detail)),
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
    let role = deserialize::<u32>(role);
    let liveness = Liveness::of(role.as_ref().err());
    let role = optional(role)?;
    let children = optional(
        deserialize::<OwnedValue>(children).and_then(|value| value.try_into().map_err(Into::into)),
    )?;
    if role.is_failed() || children.is_failed() {
        return Ok(Field::Failed("Wrapper prerequisites unreadable".into()));
    }
    if !matches!(role.present(), Some(&PANEL | &FILLER)) || children.present() != Some(&1) {
        return Ok(Field::Absent);
    }
    let (name, description) = join(
        read_text(connection, &parent, "Name", liveness),
        read_text(connection, &parent, "Description", liveness),
    )
    .await;
    Ok(select_description(&name?, description?))
}

fn select_description(name: &Field<String>, description: Field<String>) -> Field<String> {
    if name.is_failed() || description.is_failed() {
        return Field::Failed("Wrapper text unreadable".into());
    }
    if name.is_blank() && description.non_blank().is_some() {
        description
    } else {
        Field::Absent
    }
}

pub(in crate::tree) fn parent_summary(node: &RawNode) -> Field<String> {
    if node.traits.role.is_failed()
        || node.properties.name.is_failed()
        || node.properties.description.is_failed()
        || node.children.is_failed()
    {
        return Field::Failed("Wrapper prerequisites unreadable".into());
    }
    if !matches!(node.traits.role.present(), Some(&PANEL | &FILLER))
        || node.children.present().map(Vec::len) != Some(1)
    {
        return Field::Absent;
    }
    select_description(&node.properties.name, node.properties.description.clone())
}

pub(in crate::tree) async fn read_wrapper_description(
    connection: &Connection,
    node: &RawNode,
    summary: Option<&Field<String>>,
) -> Result<Field<String>, AdapterError> {
    if !needs_description(node) {
        return Ok(Field::Absent);
    }
    match summary {
        Some(description) => Ok(description.clone()),
        None => read_description(connection, &node.element).await,
    }
}

#[cfg(test)]
#[path = "wrapper_description_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "wrapper_summary_tests.rs"]
mod summary_tests;
