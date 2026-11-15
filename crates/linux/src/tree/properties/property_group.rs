//! The Accessible property group of one node, read as four `Get` calls or as
//! one `GetAll`. Either way each key ends with the outcome its own `Get`
//! would have given: a `GetAll` the application refused, or one that left a
//! key out or sent one that does not convert, is answered by the `Get` of
//! that key (L44-1, L44-3).

use std::collections::{BTreeMap, HashMap};

use futures_util::future::{join, join_all, join4};
use zbus::Connection;
use zbus::zvariant::OwnedValue;

use super::node_read::{NAME, Reply, call, deserialize};
use super::property_read::PropertyRead;
use crate::system::{ACCESSIBLE_INTERFACE, PROPERTIES_INTERFACE};
use crate::tree::AtspiElement;

pub(in crate::tree) const DESCRIPTION: &str = "Description";
pub(in crate::tree) const CHILD_COUNT: &str = "ChildCount";
pub(in crate::tree) const ACCESSIBLE_ID: &str = "AccessibleId";
const ATTRIBUTES: &str = "Attributes";
const KEYS: [&str; 4] = [NAME, DESCRIPTION, CHILD_COUNT, ACCESSIBLE_ID];

pub(in crate::tree) type Value = Result<OwnedValue, zbus::Error>;
pub(in crate::tree) type AttributeMap = BTreeMap<String, String>;
/// The attributes a group read carried, when it was asked to carry them.
pub(in crate::tree) type CarriedAttributes = Option<Result<AttributeMap, zbus::Error>>;

/// The replies of the first round: four `Get` replies, or one `GetAll`.
pub(in crate::tree) enum GroupReply {
    Separate(Box<[Reply; 4]>),
    Grouped(Reply),
}

/// The four values in `KEYS` order, and the attributes when the read was
/// asked to carry them.
pub(in crate::tree) struct GroupRead {
    pub(in crate::tree) values: [Value; 4],
    pub(in crate::tree) attributes: CarriedAttributes,
}

async fn get(connection: &Connection, element: &AtspiElement, key: &str) -> Reply {
    call(
        connection,
        element,
        PROPERTIES_INTERFACE,
        "Get",
        &(ACCESSIBLE_INTERFACE, key),
    )
    .await
}

async fn get_attributes(connection: &Connection, element: &AtspiElement) -> Reply {
    call(
        connection,
        element,
        ACCESSIBLE_INTERFACE,
        "GetAttributes",
        &(),
    )
    .await
}

/// Sends the first round of the group read.
pub(in crate::tree) async fn send_group(
    connection: &Connection,
    element: &AtspiElement,
    style: PropertyRead,
) -> GroupReply {
    match style {
        PropertyRead::Separate => {
            let (name, description, child_count, accessible_id) = join4(
                get(connection, element, NAME),
                get(connection, element, DESCRIPTION),
                get(connection, element, CHILD_COUNT),
                get(connection, element, ACCESSIBLE_ID),
            )
            .await;
            GroupReply::Separate(Box::new([name, description, child_count, accessible_id]))
        }
        PropertyRead::Grouped | PropertyRead::GroupedWithAttributes => GroupReply::Grouped(
            call(
                connection,
                element,
                PROPERTIES_INTERFACE,
                "GetAll",
                &(ACCESSIBLE_INTERFACE,),
            )
            .await,
        ),
    }
}

/// A transport failure says nothing about one method, so it stands for every
/// key just as it would have for each separate call.
fn is_transport(error: &zbus::Error) -> bool {
    matches!(
        error,
        zbus::Error::InputOutput(_) | zbus::Error::Connection(..)
    )
}

fn converts(key: &str, value: &OwnedValue) -> bool {
    if key == CHILD_COUNT {
        value.downcast_ref::<i32>().is_ok()
    } else {
        value.downcast_ref::<&str>().is_ok()
    }
}

fn attribute_map(value: OwnedValue) -> Option<AttributeMap> {
    HashMap::<String, String>::try_from(value)
        .ok()
        .map(|map| map.into_iter().collect())
}

/// Finishes the group read: each key `GetAll` did not answer in a form the
/// separate read converts is read with its own `Get`, all in flight
/// together, and the attributes with `GetAttributes` when the group did not
/// carry them.
pub(in crate::tree) async fn finish_group(
    connection: &Connection,
    element: &AtspiElement,
    reply: GroupReply,
    carry_attributes: bool,
) -> GroupRead {
    let grouped = match reply {
        GroupReply::Separate(replies) => {
            return GroupRead {
                values: (*replies).map(deserialize::<OwnedValue>),
                attributes: None,
            };
        }
        GroupReply::Grouped(reply) => deserialize::<HashMap<String, OwnedValue>>(reply),
    };
    let mut answered = match grouped {
        Err(error) if is_transport(&error) => {
            return GroupRead {
                values: KEYS.map(|_| Err(error.clone())),
                attributes: carry_attributes.then_some(Err(error)),
            };
        }
        Err(_) => HashMap::new(),
        Ok(map) => map,
    };
    let carried = if carry_attributes {
        answered.remove(ATTRIBUTES).and_then(attribute_map)
    } else {
        None
    };
    let values: [Option<OwnedValue>; 4] =
        KEYS.map(|key| answered.remove(key).filter(|value| converts(key, value)));
    let missing: Vec<usize> = (0..KEYS.len()).filter(|&i| values[i].is_none()).collect();
    let (fetched, attributes) = join(
        join_all(missing.iter().map(|&i| get(connection, element, KEYS[i]))),
        async {
            match (carry_attributes, carried) {
                (false, _) => None,
                (true, Some(map)) => Some(Ok(map)),
                (true, None) => Some(deserialize::<AttributeMap>(
                    get_attributes(connection, element).await,
                )),
            }
        },
    )
    .await;
    let mut fetched = fetched.into_iter().map(deserialize::<OwnedValue>);
    let values = values.map(|value| match value {
        Some(value) => Ok(value),
        None => fetched
            .next()
            .unwrap_or_else(|| Err(zbus::Error::Failure("property not read".to_owned()))),
    });
    GroupRead { values, attributes }
}

#[cfg(test)]
#[path = "property_group_tests.rs"]
mod tests;
