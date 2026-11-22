use std::collections::HashMap;
use std::future::poll_fn;
use std::sync::{Arc, Mutex, OnceLock, PoisonError, Weak};
use std::task::{Poll, Waker};

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
use crate::tree::properties::absence::failure;
use crate::tree::properties::failure::Failure;
use crate::tree::properties::node_read::{call, deserialize};
use crate::tree::{AtspiElement, LabelOutcome, RawNode};

const LABEL_FOR: u32 = 1;
const NULL_PATH: &str = "/org/a11y/atspi/null";
const EDITABLE_TEXT: &str = "org.a11y.atspi.EditableText";
const CHILDREN_IN_FLIGHT: usize = 4;
const COMPOSITE_CHILDREN_CAP: usize = 8;

type Address = (String, OwnedObjectPath);
type ParentKey = (String, String, String);
type Analysis = Result<Composite, Fault>;

/// What one parent proves for its editable children: its child list, the
/// editable children read, and the label proven for a sole editable child.
#[derive(Clone, Default)]
struct Composite {
    children: Vec<Address>,
    editable: Vec<Address>,
    label: Option<AtspiElement>,
}

#[derive(Clone)]
enum Fault {
    Connection(AdapterError),
    Vanished,
    Failed,
}

enum SlotState {
    Pending(Vec<Waker>),
    Ready(Analysis),
    Abandoned,
}

/// One parent's analysis, shared by every field of that parent read while it
/// is held, so concurrent reads of a form's fields read the parent once.
struct Slot(Mutex<SlotState>);

/// The read that analyses a parent; dropped unfinished, it lets the waiting
/// fields read the parent themselves.
struct Lead(Arc<Slot>);

pub(super) async fn read_composite_label(
    connection: &Connection,
    node: &RawNode,
) -> Result<LabelOutcome, AdapterError> {
    if node.element.in_web_document() || !editable_textfield(node) {
        return Ok(LabelOutcome::Unlabelled);
    }
    let parent = parent_address(connection, &node.element).await;
    let target = match parent.map(|address| owned_address(&node.element, &address)) {
        Ok(Some(parent)) => composite_of(connection, &parent)
            .await
            .map(|composite| label_of(&composite, &node.element)),
        Ok(None) => Ok(None),
        Err(error) => Err(fault(&error)),
    };
    match target {
        Ok(Some(target)) => label_text(connection, &target).await,
        Ok(None) | Err(Fault::Vanished) => Ok(LabelOutcome::Unlabelled),
        Err(Fault::Connection(error)) => Err(error),
        Err(Fault::Failed) => Ok(LabelOutcome::Failed),
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

fn fault(error: &zbus::Error) -> Fault {
    match crate::system::is_connection_failure(error) {
        true => Fault::Connection(crate::system::classify(error)),
        false if vanished(error) => Fault::Vanished,
        false => Fault::Failed,
    }
}

fn vanished(error: &zbus::Error) -> bool {
    matches!(failure(error), Failure::AppGone | Failure::Suspect)
}

fn label_of(composite: &Composite, child: &AtspiElement) -> Option<AtspiElement> {
    let label = composite.label.as_ref()?;
    let mut listed = composite.children.iter().filter(|a| is_address(child, a));
    (listed.next().is_some()
        && listed.next().is_none()
        && composite
            .editable
            .iter()
            .all(|address| is_address(child, address)))
    .then(|| label.clone())
}

async fn composite_of(connection: &Connection, parent: &AtspiElement) -> Analysis {
    let key = (
        connection.server_guid().to_string(),
        parent.bus_name().to_owned(),
        parent.path().to_owned(),
    );
    match claim(key) {
        Err(slot) => match wait_for(&slot).await {
            Some(analysis) => analysis,
            None => analyse(connection, parent).await,
        },
        Ok(lead) => {
            let analysis = analyse(connection, parent).await;
            lead.0.settle(SlotState::Ready(analysis.clone()));
            analysis
        }
    }
}

fn parents() -> &'static Mutex<HashMap<ParentKey, Weak<Slot>>> {
    static PARENTS: OnceLock<Mutex<HashMap<ParentKey, Weak<Slot>>>> = OnceLock::new();
    PARENTS.get_or_init(Mutex::default)
}

fn claim(key: ParentKey) -> Result<Lead, Arc<Slot>> {
    let mut parents = parents().lock().unwrap_or_else(PoisonError::into_inner);
    parents.retain(|_, slot| slot.strong_count() > 0);
    if let Some(slot) = parents
        .get(&key)
        .and_then(Weak::upgrade)
        .filter(|slot| !matches!(*slot.state(), SlotState::Abandoned))
    {
        return Err(slot);
    }
    let slot = Arc::new(Slot(Mutex::new(SlotState::Pending(Vec::new()))));
    parents.insert(key, Arc::downgrade(&slot));
    Ok(Lead(slot))
}

impl Slot {
    fn state(&self) -> std::sync::MutexGuard<'_, SlotState> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn settle(&self, outcome: SlotState) {
        let mut state = self.state();
        if let SlotState::Pending(wakers) = &mut *state {
            let wakers = std::mem::take(wakers);
            *state = outcome;
            drop(state);
            wakers.into_iter().for_each(Waker::wake);
        }
    }
}

impl Drop for Lead {
    fn drop(&mut self) {
        self.0.settle(SlotState::Abandoned);
    }
}

async fn wait_for(slot: &Slot) -> Option<Analysis> {
    poll_fn(|context| match &mut *slot.state() {
        SlotState::Ready(analysis) => Poll::Ready(Some(analysis.clone())),
        SlotState::Abandoned => Poll::Ready(None),
        SlotState::Pending(wakers) => {
            if !wakers.iter().any(|waker| waker.will_wake(context.waker())) {
                wakers.push(context.waker().clone());
            }
            Poll::Pending
        }
    })
    .await
}

async fn analyse(connection: &Connection, parent: &AtspiElement) -> Analysis {
    read_composite(connection, parent)
        .await
        .map_err(|error| fault(&error))
}

async fn read_composite(
    connection: &Connection,
    parent: &AtspiElement,
) -> Result<Composite, zbus::Error> {
    let (relations, children) = try_join(
        read_relations(connection, parent),
        accessible::<Vec<Address>>(connection, parent, "GetChildren"),
    )
    .await?;
    if children.len() > COMPOSITE_CHILDREN_CAP {
        return Ok(Composite::default());
    }
    let Some(elements) = children
        .iter()
        .map(|address| owned_address(parent, address))
        .collect::<Option<Vec<_>>>()
    else {
        return Ok(Composite::default());
    };
    let targets: Vec<&Address> = relation_targets(&relations, LABELLED_BY).collect();
    let reads = read_children(connection, &elements, targets.is_empty()).await?;
    let editable: Vec<Address> = reads
        .iter()
        .filter(|(_, editable, _)| *editable)
        .map(|(index, _, _)| children[*index].clone())
        .collect();
    if editable.len() > 1 {
        return Ok(Composite::default());
    }
    let label = match targets.as_slice() {
        [target] => proven_label(connection, parent, target, false).await?,
        [] => {
            let mut combos = reads
                .iter()
                .filter(|(_, editable, combo)| *combo && !*editable)
                .map(|(index, _, _)| &children[*index]);
            match (combos.next(), combos.next()) {
                (Some(combo), None) => shared_label(connection, parent, combo).await?,
                _ => None,
            }
        }
        _ => None,
    };
    Ok(Composite {
        children,
        editable,
        label,
    })
}

/// Reads the children a few at a time and stops once two of them are
/// editable, since no field of such a parent can own its label. A child that
/// vanished is absent.
async fn read_children(
    connection: &Connection,
    children: &[AtspiElement],
    need_role: bool,
) -> Result<Vec<(usize, bool, bool)>, zbus::Error> {
    let mut reads = Vec::with_capacity(children.len());
    for (start, chunk) in (0..)
        .step_by(CHILDREN_IN_FLIGHT)
        .zip(children.chunks(CHILDREN_IN_FLIGHT))
    {
        let chunk_reads = collect_bounded(
            CHILDREN_IN_FLIGHT,
            chunk
                .iter()
                .map(|child| read_child(connection, child, need_role)),
        )
        .await;
        for (index, read) in (start..).zip(chunk_reads) {
            match read {
                Ok((editable, combo)) => reads.push((index, editable, combo)),
                Err(error) if vanished(&error) => {}
                Err(error) => return Err(error),
            }
        }
        if reads.iter().filter(|(_, editable, _)| *editable).count() > 1 {
            break;
        }
    }
    Ok(reads)
}

async fn read_child(
    connection: &Connection,
    child: &AtspiElement,
    need_role: bool,
) -> Result<(bool, bool), zbus::Error> {
    let (editability, role) = join(
        try_join(
            accessible::<Vec<u32>>(connection, child, "GetState"),
            accessible::<Vec<String>>(connection, child, "GetInterfaces"),
        ),
        async {
            if need_role {
                accessible::<u32>(connection, child, "GetRole").await
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
    combo: &Address,
) -> Result<Option<AtspiElement>, zbus::Error> {
    let Some(combo) = owned_address(parent, combo) else {
        return Ok(None);
    };
    let relations = read_relations(connection, &combo).await?;
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
