use std::collections::BTreeMap;

use agent_desktop_core::AdapterError;
use futures_util::future::{join, join3, join4};
use zbus::export::serde::Serialize;
use zbus::zvariant::{DynamicDeserialize, DynamicType, OwnedValue};
use zbus::{Connection, Message};

use super::absence::gone_error;
use super::interface_reads::{read_actions, read_extents};
use super::liveness::Liveness;
use super::property_group::{CarriedAttributes, GroupRead, finish_group, send_group};
use super::property_read::PropertyRead;
use super::read_purpose::ReadPurpose;
use crate::system::{ACCESSIBLE_INTERFACE, PROPERTIES_INTERFACE};
use crate::tree::{AtspiElement, ChildAddress, Field, NodeProperties, NodeTraits, RawNode};

pub(crate) const COMPONENT_INTERFACE: &str = "org.a11y.atspi.Component";
pub(in crate::tree) const NAME: &str = "Name";
const NULL_PATH: &str = "/org/a11y/atspi/null";

pub(in crate::tree) type Reply = Result<Message, zbus::Error>;

pub(in crate::tree) fn deserialize<T>(reply: Reply) -> Result<T, zbus::Error>
where
    T: for<'de> DynamicDeserialize<'de>,
{
    reply?.body().deserialize()
}

pub(crate) async fn call<B>(
    connection: &Connection,
    element: &AtspiElement,
    interface: &str,
    method: &str,
    body: &B,
) -> Reply
where
    B: Serialize + DynamicType,
{
    connection
        .call_method(
            Some(element.bus_name()),
            element.path(),
            Some(interface),
            method,
            body,
        )
        .await
}

/// A read's outcome as a field. An error that shows the application or the
/// object gone fails the node read; any other failure, including a name from
/// the suspect family on a node that still answers, stays on its field.
pub(in crate::tree) fn field<T>(
    result: Result<T, zbus::Error>,
    liveness: Liveness,
) -> Result<Field<T>, AdapterError> {
    match result {
        Ok(value) => Ok(Field::Present(value)),
        Err(error) if crate::system::is_connection_failure(&error) => {
            Err(crate::system::classify(&error))
        }
        Err(error) => match gone_error(&error, liveness) {
            Some(gone) => Err(gone),
            None => Ok(Field::Failed(error.to_string())),
        },
    }
}

pub(in crate::tree) fn string_value(value: &OwnedValue) -> Field<String> {
    match value.downcast_ref::<&str>() {
        Ok(text) => Field::Present(text.to_owned()),
        Err(error) => Field::Failed(error.to_string()),
    }
}

fn count_value(value: &OwnedValue) -> Field<i32> {
    match value.downcast_ref::<i32>() {
        Ok(count) => Field::Present(count),
        Err(error) => Field::Failed(error.to_string()),
    }
}

fn property_field<T>(
    value: Result<OwnedValue, zbus::Error>,
    convert: fn(&OwnedValue) -> Field<T>,
    liveness: Liveness,
) -> Result<Field<T>, AdapterError> {
    Ok(match field(value, liveness)? {
        Field::Present(value) => convert(&value),
        Field::Absent => Field::Absent,
        Field::Failed(detail) => Field::Failed(detail),
    })
}

fn group_properties(
    read: GroupRead,
    liveness: Liveness,
) -> Result<(NodeProperties, CarriedAttributes), AdapterError> {
    let [name, description, child_count, accessible_id] = read.values;
    Ok((
        NodeProperties {
            name: property_field(name, string_value, liveness)?,
            description: property_field(description, string_value, liveness)?,
            child_count: property_field(child_count, count_value, liveness)?,
            accessible_id: property_field(accessible_id, string_value, liveness)?,
        },
        read.attributes,
    ))
}

pub(in crate::tree) async fn get_one<T>(
    connection: &Connection,
    element: &AtspiElement,
    key: &'static str,
    convert: fn(&OwnedValue) -> Field<T>,
    liveness: Liveness,
) -> Result<Field<T>, AdapterError> {
    let reply = call(
        connection,
        element,
        PROPERTIES_INTERFACE,
        "Get",
        &(ACCESSIBLE_INTERFACE, key),
    )
    .await;
    let read = field(deserialize::<OwnedValue>(reply), liveness)?;
    Ok(match read {
        Field::Present(value) => convert(&value),
        Field::Absent => Field::Failed(format!("Properties.Get({key}) failed")),
        Field::Failed(detail) => Field::Failed(format!("Properties.Get({key}) failed: {detail}")),
    })
}

pub(in crate::tree) fn read_children(
    reply: Reply,
    liveness: Liveness,
) -> Result<Field<Vec<ChildAddress>>, AdapterError> {
    let raw = field(
        deserialize::<Vec<(String, zbus::zvariant::OwnedObjectPath)>>(reply),
        liveness,
    )?;
    Ok(raw.map(|entries| {
        entries
            .into_iter()
            .filter(|(bus, path)| !bus.is_empty() && path.as_str() != NULL_PATH)
            .map(|(bus, path)| (bus, path.to_string()))
            .collect()
    }))
}

fn selected<T>(reply: Option<Reply>, liveness: Liveness) -> Result<Field<T>, AdapterError>
where
    T: for<'de> DynamicDeserialize<'de>,
{
    match reply {
        Some(reply) => field(deserialize(reply), liveness),
        None => Ok(Field::Failed("field not requested".to_owned())),
    }
}

/// Reads one node with the property group read separately. L41-7: a bulk
/// read can crash a toolkit, so every caller that has not established the
/// toolkit uses this.
pub(crate) async fn read_node_for(
    connection: &Connection,
    element: &AtspiElement,
    purpose: ReadPurpose,
) -> Result<(RawNode, Liveness), AdapterError> {
    read_node_with(connection, element, purpose, PropertyRead::Separate).await
}

/// Reads one node. All fixed calls are in flight together; extents and
/// actions follow only when the interface list names them, and a grouped
/// property read follows with the separate reads it could not stand for.
/// The node's own `GetRole` outcome, which every live node answers, decides
/// what a failure of any other call means: an error that shows the
/// application or the object gone fails the read; any other failure stays on
/// the field it hit. The second value is `Alive`, or `TimedOut` when the role
/// call did not answer in time, which says nothing about the object.
pub(in crate::tree) async fn read_node_with(
    connection: &Connection,
    element: &AtspiElement,
    purpose: ReadPurpose,
    style: PropertyRead,
) -> Result<(RawNode, Liveness), AdapterError> {
    let accessible = |method: &'static str, needed| async move {
        if needed {
            Some(call(connection, element, ACCESSIBLE_INTERFACE, method, &()).await)
        } else {
            None
        }
    };
    let group_needed = purpose.property_group();
    let carry_attributes = group_needed && style == PropertyRead::GroupedWithAttributes;
    let (first, second) = join(
        join4(
            async {
                if group_needed {
                    Some(send_group(connection, element, style).await)
                } else {
                    None
                }
            },
            accessible("GetRole", true),
            accessible("GetRoleName", purpose.vocabulary()),
            accessible("GetState", purpose != ReadPurpose::Shape),
        ),
        join3(
            accessible(
                "GetInterfaces",
                !matches!(
                    purpose,
                    ReadPurpose::Structure | ReadPurpose::Shape | ReadPurpose::Gate
                ),
            ),
            accessible("GetAttributes", purpose.vocabulary() && !carry_attributes),
            accessible("GetChildren", purpose.children()),
        ),
    )
    .await;
    let (group, role, role_name, state) = first;
    let (interfaces, attributes, child_reply) = second;
    let role = role.map(deserialize::<u32>);
    let liveness = Liveness::of(role.as_ref().and_then(|read| read.as_ref().err()));
    let (properties, carried) = match group {
        Some(reply) => group_properties(
            finish_group(connection, element, reply, carry_attributes).await,
            liveness,
        )?,
        None => {
            let mut properties = NodeProperties::failed("field not requested");
            if matches!(purpose, ReadPurpose::Frame | ReadPurpose::Structure) {
                properties.name =
                    get_one(connection, element, NAME, string_value, liveness).await?;
            }
            (properties, None)
        }
    };
    let interfaces = selected(interfaces, liveness)?;
    let (extents, actions) = join(
        async {
            if purpose.bounds() {
                read_extents(connection, element, &interfaces, liveness).await
            } else {
                Ok(Field::Failed("field not requested".to_owned()))
            }
        },
        async {
            if purpose.actions() {
                read_actions(connection, element, &interfaces, liveness).await
            } else {
                Ok(Field::Failed("field not requested".to_owned()))
            }
        },
    )
    .await;
    let node = RawNode {
        element: element.clone(),
        properties,
        traits: NodeTraits {
            role: role.map_or_else(
                || Ok(Field::Failed("role not read".to_owned())),
                |read| field(read, liveness),
            )?,
            role_name: selected(role_name, liveness)?,
            states: selected(state, liveness)?,
            interfaces,
        },
        attributes: match carried {
            Some(read) => field(read, liveness)?,
            None => selected::<BTreeMap<String, String>>(attributes, liveness)?,
        },
        extents: extents?,
        actions: actions?,
        children: match child_reply {
            Some(reply) => read_children(reply, liveness)?,
            None => Field::Failed("field not requested".to_owned()),
        },
    };
    Ok((node, liveness))
}

pub(crate) async fn read_node(
    connection: &Connection,
    element: &AtspiElement,
) -> Result<RawNode, AdapterError> {
    read_node_for(connection, element, ReadPurpose::Snapshot)
        .await
        .map(|(node, _)| node)
}

#[cfg(test)]
#[path = "node_read_tests.rs"]
mod tests;
