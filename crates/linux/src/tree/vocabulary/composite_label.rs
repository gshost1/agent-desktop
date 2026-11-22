use agent_desktop_core::{AdapterError, Role};
use futures_util::future::{join, try_join};
use zbus::Connection;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

use super::label_read::{LABELLED_BY, RelationSet, label_text, read_relations};
use super::role_numbers::{COMBO_BOX, LABEL};
use super::roles::role_of;
use super::state_bits::{EDITABLE, StateBits};
use crate::system::{ACCESSIBLE_INTERFACE, PROPERTIES_INTERFACE};
use crate::tree::element::bounded::collect_bounded;
use crate::tree::properties::node_read::{call, deserialize};
use crate::tree::{AtspiElement, LabelOutcome, RawNode};

const LABEL_FOR: u32 = 1;
const NULL_PATH: &str = "/org/a11y/atspi/null";
const EDITABLE_TEXT: &str = "org.a11y.atspi.EditableText";
const CHILDREN_IN_FLIGHT: usize = 8;

type Address = (String, OwnedObjectPath);

pub(super) async fn read_composite_label(
    connection: &Connection,
    node: &RawNode,
) -> Result<LabelOutcome, AdapterError> {
    if !editable_textfield(node) {
        return Ok(LabelOutcome::Unlabelled);
    }
    match composite_target(connection, &node.element).await {
        Ok(Some(target)) => label_text(connection, &target).await,
        Ok(None) => Ok(LabelOutcome::Unlabelled),
        Err(error) if crate::system::is_connection_failure(&error) => {
            Err(crate::system::classify(&error))
        }
        Err(_) => Ok(LabelOutcome::Failed),
    }
}

fn editable_textfield(node: &RawNode) -> bool {
    let (Some(role), Some(states)) = (node.traits.role.present(), node.traits.states.present())
    else {
        return false;
    };
    StateBits::new(states).has(EDITABLE)
        && role_of(
            *role,
            &node.traits.states,
            &node.traits.interfaces,
            &node.attributes,
        ) == Role::TextField
}

async fn composite_target(
    connection: &Connection,
    child: &AtspiElement,
) -> Result<Option<AtspiElement>, zbus::Error> {
    let Some(parent) = owned_address(child, &parent_address(connection, child).await?) else {
        return Ok(None);
    };
    let (relations, children) = try_join(
        read_relations(connection, &parent),
        accessible::<Vec<Address>>(connection, &parent, "GetChildren"),
    )
    .await?;
    let Some(siblings) = siblings(child, &children) else {
        return Ok(None);
    };
    let targets: Vec<&Address> = relation_targets(&relations, LABELLED_BY).collect();
    let sibling_reads = collect_bounded(
        CHILDREN_IN_FLIGHT,
        siblings
            .iter()
            .map(|sibling| read_sibling(connection, sibling, targets.is_empty())),
    )
    .await;
    let sibling_reads: Vec<_> = sibling_reads.into_iter().collect::<Result<_, _>>()?;
    if sibling_reads.iter().any(|(editable, _)| *editable) {
        return Ok(None);
    }
    match targets.as_slice() {
        [target] => proven_label(connection, &parent, target, false).await,
        [] => {
            let mut combos = siblings
                .iter()
                .zip(sibling_reads)
                .filter_map(|(sibling, (_, combo))| combo.then_some(sibling));
            match (combos.next(), combos.next()) {
                (Some(combo), None) => shared_label(connection, &parent, combo).await,
                _ => Ok(None),
            }
        }
        _ => Ok(None),
    }
}

async fn read_sibling(
    connection: &Connection,
    sibling: &AtspiElement,
    need_role: bool,
) -> Result<(bool, bool), zbus::Error> {
    let (editability, role) = join(
        try_join(
            accessible::<Vec<u32>>(connection, sibling, "GetState"),
            accessible::<Vec<String>>(connection, sibling, "GetInterfaces"),
        ),
        async {
            if need_role {
                accessible::<u32>(connection, sibling, "GetRole").await
            } else {
                Ok(0)
            }
        },
    )
    .await;
    let (states, interfaces) = editability?;
    let editable = StateBits::new(&states).has(EDITABLE)
        || interfaces.iter().any(|name| name == EDITABLE_TEXT);
    Ok((editable, role? == COMBO_BOX))
}

async fn shared_label(
    connection: &Connection,
    parent: &AtspiElement,
    combo: &AtspiElement,
) -> Result<Option<AtspiElement>, zbus::Error> {
    let relations = read_relations(connection, combo).await?;
    let mut targets = relation_targets(&relations, LABELLED_BY);
    let (Some(target), None) = (targets.next(), targets.next()) else {
        return Ok(None);
    };
    proven_label(connection, parent, target, true).await
}

async fn proven_label(
    connection: &Connection,
    parent: &AtspiElement,
    target: &Address,
    require_label_for: bool,
) -> Result<Option<AtspiElement>, zbus::Error> {
    let Some(label) = owned_address(parent, target) else {
        return Ok(None);
    };
    let (role, relations) = try_join(
        accessible::<u32>(connection, &label, "GetRole"),
        read_relations(connection, &label),
    )
    .await?;
    if role != LABEL {
        return Ok(None);
    }
    let mut targets = relation_targets(&relations, LABEL_FOR);
    Ok(match (targets.next(), targets.next()) {
        (Some(target), None) if is_address(parent, target) => Some(label),
        (None, None) if !require_label_for => Some(label),
        _ => None,
    })
}

fn siblings(child: &AtspiElement, children: &[Address]) -> Option<Vec<AtspiElement>> {
    let mut matches = children.iter().filter(|address| is_address(child, address));
    if matches.next().is_none() || matches.next().is_some() {
        return None;
    }
    children
        .iter()
        .filter(|address| !is_address(child, address))
        .map(|address| owned_address(child, address))
        .collect()
}

fn relation_targets(relations: &RelationSet, kind: u32) -> impl Iterator<Item = &Address> {
    relations
        .iter()
        .filter(move |(relation, _)| *relation == kind)
        .flat_map(|(_, targets)| targets)
}

fn is_address(element: &AtspiElement, (bus, path): &Address) -> bool {
    bus == element.bus_name() && path.as_str() == element.path()
}

fn owned_address(element: &AtspiElement, (bus, path): &Address) -> Option<AtspiElement> {
    (bus == element.bus_name() && path.as_str() != NULL_PATH)
        .then(|| AtspiElement::new(bus, path.as_str(), element.pid()))
}

async fn parent_address(
    connection: &Connection,
    element: &AtspiElement,
) -> Result<Address, zbus::Error> {
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
    Ok(Address::try_from(value)?)
}

async fn accessible<T>(
    connection: &Connection,
    element: &AtspiElement,
    method: &str,
) -> Result<T, zbus::Error>
where
    T: for<'de> zbus::zvariant::DynamicDeserialize<'de>,
{
    deserialize(call(connection, element, ACCESSIBLE_INTERFACE, method, &()).await)
}

#[cfg(test)]
#[path = "composite_label_tests.rs"]
mod tests;
