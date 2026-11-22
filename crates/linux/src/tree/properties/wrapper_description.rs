use agent_desktop_core::AdapterError;
use futures_util::future::join;
use zbus::Connection;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

use super::liveness::Liveness;
use super::node_read::{call, deserialize, get_one, read_children, string_value};
use super::read_purpose::ReadPurpose;
use super::wrapper_parent::WrapperParent;
use crate::system::{ACCESSIBLE_INTERFACE, PROPERTIES_INTERFACE};
use crate::tree::vocabulary::role_numbers::{FILLER, PANEL};
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
    !node.element.in_web_document() && node.properties.description.is_blank()
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
            ACCESSIBLE_INTERFACE,
            "GetChildren",
            &(),
        ),
    )
    .await;
    let role = deserialize::<u32>(role);
    let liveness = Liveness::of(role.as_ref().err());
    let role = optional(role)?;
    let children = read_children(children, Liveness::Alive)?;
    read_proven_description(connection, element, &parent, (role, liveness), children).await
}

async fn read_proven_description(
    connection: &Connection,
    element: &AtspiElement,
    parent: &AtspiElement,
    role: (Field<u32>, Liveness),
    children: Field<Vec<(String, String)>>,
) -> Result<Field<String>, AdapterError> {
    let (role, liveness) = role;
    if role.is_failed() || children.is_failed() {
        return Ok(Field::Failed("Wrapper prerequisites unreadable".into()));
    }
    if !matches!(role.present(), Some(&PANEL | &FILLER))
        || !children.present().is_some_and(|children| {
            children.len() == 1
                && children[0].0 == element.bus_name()
                && children[0].1 == element.path()
        })
    {
        return Ok(Field::Absent);
    }
    read_parent_text(connection, parent, liveness).await
}

async fn read_from_children(
    connection: &Connection,
    element: &AtspiElement,
    parent: &AtspiElement,
    children: &[(String, OwnedObjectPath)],
) -> Result<Field<String>, AdapterError> {
    if children.len() > 1 {
        return Ok(Field::Absent);
    }
    let role =
        deserialize::<u32>(call(connection, parent, ACCESSIBLE_INTERFACE, "GetRole", &()).await);
    let liveness = Liveness::of(role.as_ref().err());
    read_proven_description(
        connection,
        element,
        parent,
        (optional(role)?, liveness),
        Field::Present(
            children
                .iter()
                .map(|(bus, path)| (bus.clone(), path.to_string()))
                .collect(),
        ),
    )
    .await
}

async fn read_parent_text(
    connection: &Connection,
    parent: &AtspiElement,
    liveness: Liveness,
) -> Result<Field<String>, AdapterError> {
    let (name, description) = join(
        read_text(connection, parent, "Name", liveness),
        read_text(connection, parent, "Description", liveness),
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

pub(in crate::tree) fn parent_summary(
    node: &RawNode,
    purpose: ReadPurpose,
) -> Option<Field<String>> {
    purpose.children().then(|| summarize_parent(node))
}

fn summarize_parent(node: &RawNode) -> Field<String> {
    if node.traits.role.is_failed() || node.children.is_failed() {
        return Field::Failed("Wrapper prerequisites unreadable".into());
    }
    if !matches!(node.traits.role.present(), Some(&PANEL | &FILLER))
        || node.children.present().map(Vec::len) != Some(1)
    {
        return Field::Absent;
    }
    select_description(&node.properties.name, node.properties.description.clone())
}

pub(in crate::tree) fn listing_parent(node: &RawNode, child: &AtspiElement) -> WrapperParent {
    let evidence = if node.element.bus_name() != child.bus_name() {
        Field::Absent
    } else if node.traits.role.is_failed() || node.children.is_failed() {
        Field::Failed("Wrapper prerequisites unreadable".into())
    } else if matches!(node.traits.role.present(), Some(&PANEL | &FILLER))
        && node.children.present().map(Vec::len) == Some(1)
    {
        return WrapperParent::Listing(node.element.clone());
    } else {
        Field::Absent
    };
    WrapperParent::Known(evidence)
}

pub(in crate::tree) async fn read_wrapper_description(
    connection: &Connection,
    node: &RawNode,
    summary: Option<&WrapperParent>,
) -> Result<Field<String>, AdapterError> {
    if !needs_description(node) {
        return Ok(Field::Absent);
    }
    match summary {
        Some(WrapperParent::Known(description)) => Ok(description.clone()),
        Some(WrapperParent::Listing(parent)) => {
            read_parent_text(connection, parent, Liveness::Alive).await
        }
        Some(WrapperParent::Children(parent, children)) => {
            read_from_children(connection, &node.element, parent, children).await
        }
        None => read_description(connection, &node.element).await,
    }
}

#[cfg(test)]
#[path = "wrapper_description_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "wrapper_summary_tests.rs"]
mod summary_tests;

#[cfg(test)]
#[path = "wrapper_listing_tests.rs"]
mod listing_tests;
