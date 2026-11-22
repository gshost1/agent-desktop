use agent_desktop_core::{AdapterError, tree::roles::is_interactive_role};
use futures_util::future::join4;
use zbus::Connection;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

use super::liveness::Liveness;
use super::node_read::{call, deserialize, field, get_one, string_value};
use crate::system::{ACCESSIBLE_INTERFACE, PROPERTIES_INTERFACE};
use crate::tree::vocabulary::label_read::read_label;
use crate::tree::vocabulary::role_numbers::{FILLER, PANEL};
use crate::tree::vocabulary::roles::role_of;
use crate::tree::{AtspiElement, Field, LabelOutcome, RawNode};

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
    let parent = match field(parent(connection, element).await, Liveness::Alive)? {
        Field::Present(Some(parent)) => parent,
        Field::Failed(detail) => return Ok(Field::Failed(detail)),
        _ => return Ok(Field::Absent),
    };
    let (role, name, description, children) = join4(
        call(connection, &parent, ACCESSIBLE_INTERFACE, "GetRole", &()),
        get_one(connection, &parent, "Name", string_value, Liveness::Alive),
        get_one(
            connection,
            &parent,
            "Description",
            string_value,
            Liveness::Alive,
        ),
        call(
            connection,
            &parent,
            PROPERTIES_INTERFACE,
            "Get",
            &(ACCESSIBLE_INTERFACE, "ChildCount"),
        ),
    )
    .await;
    let role = field(deserialize::<u32>(role), Liveness::Alive)?;
    let children = field(
        deserialize::<OwnedValue>(children).and_then(|value| value.try_into().map_err(Into::into)),
        Liveness::Alive,
    )?;
    Ok(select_description(&role, &name?, description?, &children))
}

fn select_description(
    role: &Field<u32>,
    name: &Field<String>,
    description: Field<String>,
    children: &Field<i32>,
) -> Field<String> {
    if matches!(role.present(), Some(&PANEL | &FILLER))
        && name.is_blank()
        && children.present() == Some(&1)
    {
        return description;
    }
    if role.is_failed() || name.is_failed() || children.is_failed() {
        return Field::Failed("wrapper description could not be established".to_owned());
    }
    Field::Absent
}

pub(super) async fn inherit_description(
    connection: &Connection,
    node: &mut RawNode,
) -> Result<(), AdapterError> {
    if !needs_description(node)
        || read_label(connection, &node.element).await? != LabelOutcome::Unlabelled
    {
        return Ok(());
    }
    let description = read_description(connection, &node.element).await?;
    if !description.is_blank() {
        node.properties.description = description;
    }
    Ok(())
}

#[cfg(test)]
#[path = "wrapper_description_tests.rs"]
mod tests;
