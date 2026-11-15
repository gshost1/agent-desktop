use std::collections::{HashMap, HashSet};

use agent_desktop_core::{AdapterError, Deadline, ErrorCode};
use futures_util::future::join;
use futures_util::stream::{self, StreamExt};
use zbus::Connection;

use crate::system::{REGISTRY_ROOT_PATH, process_id, within};
use crate::tree::capabilities::value_read::read_value;
use crate::tree::element::ancestry;
use crate::tree::element::bounded::collect_bounded;
use crate::tree::properties::liveness::Liveness;
use crate::tree::properties::node_read::read_node_with;
use crate::tree::properties::property_read::PropertyRead;
use crate::tree::properties::read_purpose::ReadPurpose;
use crate::tree::properties::toolkit_trust::{property_read, read_toolkit};
use crate::tree::vocabulary::label_read::read_label_if_needed;
use crate::tree::{AtspiElement, Expansion, ObservedRead, RawNode, WalkLimits};

const NODES_IN_FLIGHT: usize = 32;
const PID_LOOKUPS_IN_FLIGHT: usize = 64;
const TOOLKIT_LOOKUPS_IN_FLIGHT: usize = 8;

/// Stands in for the process id of a bus name whose lookup failed. Process
/// id 0 is never an application, so a child carrying it is recorded as
/// unreadable instead of being read.
pub(in crate::tree) const UNKNOWN_PID: u32 = 0;
const UNIDENTIFIED: &str = "The owning process of the bus name could not be identified";

pub(in crate::tree) async fn learn_pids<'a>(
    connection: &Connection,
    pids: &mut HashMap<String, u32>,
    reads: impl Iterator<Item = &'a RawNode>,
) -> Result<(), AdapterError> {
    let unseen: HashSet<&str> = reads
        .filter_map(|node| node.children.present())
        .flatten()
        .map(|(bus, _)| bus.as_str())
        .filter(|bus| !pids.contains_key(*bus))
        .collect();
    let lookups = unseen
        .into_iter()
        .map(|bus| process_id(connection, bus.to_owned()));
    record_pids(pids, collect_bounded(PID_LOOKUPS_IN_FLIGHT, lookups).await)
}

/// Stores each resolved process id. A lookup that failed because the bus
/// name is gone or unanswerable leaves that name unrecorded, so its children
/// are kept as unreadable nodes; budget exhaustion and bus loss fail the level.
pub(in crate::tree) fn record_pids(
    pids: &mut HashMap<String, u32>,
    lookups: Vec<(String, Result<u32, AdapterError>)>,
) -> Result<(), AdapterError> {
    for (bus, pid) in lookups {
        match pid {
            Ok(pid) => {
                pids.insert(bus, pid);
            }
            Err(error) if matches!(error.code, ErrorCode::Timeout | ErrorCode::Internal) => {
                return Err(error);
            }
            Err(_) => {}
        }
    }
    Ok(())
}

/// Learns, for each bus name of the level not seen before, how its nodes read
/// the property group: from the toolkit its application root names, read one
/// property at a time. A root that cannot be read keeps the separate reads;
/// bus loss fails the level.
pub(in crate::tree) async fn learn_property_reads(
    connection: &Connection,
    styles: &mut HashMap<String, PropertyRead>,
    level: &[AtspiElement],
) -> Result<(), AdapterError> {
    let mut unseen: Vec<&AtspiElement> = Vec::new();
    for element in level {
        let bus = element.bus_name();
        if element.pid() != UNKNOWN_PID
            && !styles.contains_key(bus)
            && !unseen.iter().any(|seen| seen.bus_name() == bus)
        {
            unseen.push(element);
        }
    }
    let lookups = unseen.into_iter().map(|element| async move {
        let root = AtspiElement::new(element.bus_name(), REGISTRY_ROOT_PATH, element.pid());
        (
            element.bus_name().to_owned(),
            read_toolkit(connection, &root).await,
        )
    });
    for (bus, toolkit) in collect_bounded(TOOLKIT_LOOKUPS_IN_FLIGHT, lookups).await {
        styles.insert(bus, property_read(&toolkit?));
    }
    Ok(())
}

pub(in crate::tree) fn child_elements(
    node: &RawNode,
    pids: &HashMap<String, u32>,
) -> Vec<AtspiElement> {
    let ancestry = ancestry::child(&node.element, node.traits.role.present().copied());
    node.children
        .present()
        .into_iter()
        .flatten()
        .map(|(bus, path)| {
            let pid = pids.get(bus).copied().unwrap_or(UNKNOWN_PID);
            let mut element = AtspiElement::new(bus.as_str(), path.as_str(), pid);
            element.set_ancestry(ancestry.clone());
            element
        })
        .collect()
}

/// Reads the levels of one walk: the shared connection, the process ids
/// learned so far, and what a node read must carry.
pub(in crate::tree) struct LevelReader<'a> {
    connection: &'a Connection,
    pids: HashMap<String, u32>,
    max_field_bytes: usize,
    deadline: Deadline,
    at_root: bool,
    purpose: ReadPurpose,
    property_reads: HashMap<String, PropertyRead>,
}

impl<'a> LevelReader<'a> {
    pub(in crate::tree) fn new(
        connection: &'a Connection,
        root: &AtspiElement,
        limits: &WalkLimits,
        deadline: Deadline,
    ) -> Self {
        Self {
            connection,
            pids: HashMap::from([(root.bus_name().to_owned(), root.pid())]),
            max_field_bytes: limits.max_field_bytes,
            deadline,
            at_root: true,
            purpose: ReadPurpose::Snapshot,
            property_reads: HashMap::new(),
        }
    }

    pub(in crate::tree) fn for_purpose(mut self, purpose: ReadPurpose) -> Self {
        self.purpose = purpose;
        self
    }

    /// Reads one level of the tree with at most `NODES_IN_FLIGHT` nodes
    /// outstanding at a time, so a wide level costs one round of calls. Below the root a node that is gone,
    /// or whose process id could not be found, is kept as an unreadable node
    /// marked gone. Transport failures abort the walk at every depth.
    pub(in crate::tree) fn expand(
        &mut self,
        level: &[AtspiElement],
    ) -> Result<Vec<Expansion<ObservedRead, AtspiElement>>, AdapterError> {
        let at_root = std::mem::replace(&mut self.at_root, false);
        let (connection, max_bytes) = (self.connection, self.max_field_bytes);
        let pids = &mut self.pids;
        let styles = &mut self.property_reads;
        let purpose = self.purpose;
        within(self.deadline, async {
            if purpose.property_group() {
                learn_property_reads(connection, styles, level).await?;
            }
            let styles = &*styles;
            let reads: Vec<Result<ObservedRead, AdapterError>> =
                stream::iter(level.iter().map(|element| async move {
                    if !at_root && element.pid() == UNKNOWN_PID {
                        Err(AdapterError::new(ErrorCode::ElementNotFound, UNIDENTIFIED))
                    } else {
                        let style = styles
                            .get(element.bus_name())
                            .copied()
                            .unwrap_or(PropertyRead::Separate);
                        read_observed(connection, element, max_bytes, purpose, style)
                            .await
                            .map(|(read, _)| read)
                    }
                }))
                .buffered(NODES_IN_FLIGHT)
                .collect()
                .await;
            let mut nodes = Vec::with_capacity(reads.len());
            for (element, read) in level.iter().zip(reads) {
                match read {
                    Ok(read) => nodes.push((read, false)),
                    Err(error)
                        if at_root
                            || !matches!(
                                error.code,
                                ErrorCode::ElementNotFound | ErrorCode::AppNotFound
                            ) =>
                    {
                        return Err(error);
                    }
                    Err(error) => nodes.push((
                        ObservedRead::unreadable(element.clone(), &error.message),
                        true,
                    )),
                }
            }
            learn_pids(connection, pids, nodes.iter().map(|(read, _)| &read.node)).await?;
            Ok(nodes
                .into_iter()
                .map(|(read, gone)| {
                    let children = child_elements(&read.node, pids);
                    Expansion {
                        children_unread: read.node.children.is_failed(),
                        payload: read,
                        children,
                        gone,
                    }
                })
                .collect())
        })
    }
}

/// The walk's own per-node read, with the liveness the node's `GetRole` showed.
pub(in crate::tree) async fn read_observed(
    connection: &Connection,
    element: &AtspiElement,
    max_field_bytes: usize,
    purpose: ReadPurpose,
    style: PropertyRead,
) -> Result<(ObservedRead, Liveness), AdapterError> {
    let (node, liveness) = read_node_with(connection, element, purpose, style).await?;
    let (label, value) = join(
        async {
            if purpose.identity() {
                read_label_if_needed(connection, &node).await
            } else {
                Ok(crate::tree::LabelOutcome::Failed)
            }
        },
        async {
            if purpose.value() {
                read_value(connection, &node, max_field_bytes).await
            } else {
                Ok(crate::tree::capabilities::value_read::ValueRead {
                    value: crate::tree::Field::Failed("field not requested".to_owned()),
                    truncated: false,
                })
            }
        },
    )
    .await;
    Ok((
        ObservedRead {
            node,
            label: label?,
            value: value?,
            evidence_bounds: None,
        },
        liveness,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Accessible;

    #[zbus::interface(name = "org.a11y.atspi.Accessible")]
    impl Accessible {
        fn get_role(&self) -> u32 {
            crate::tree::vocabulary::role_numbers::FRAME
        }

        fn get_children(&self) -> Vec<(String, zbus::zvariant::OwnedObjectPath)> {
            vec![(":1.1".to_owned(), "/child".try_into().unwrap())]
        }
    }

    #[test]
    fn descendant_connection_loss_aborts_the_walk_instead_of_marking_a_node_gone() {
        use zbus::connection::Builder;
        let (near, far) = std::os::unix::net::UnixStream::pair().unwrap();
        let (connection, server) = zbus::block_on(futures_lite::future::zip(
            Builder::async_io_unix_stream(near).p2p().build(),
            Builder::async_io_unix_stream(far)
                .server(zbus::Guid::generate())
                .unwrap()
                .p2p()
                .serve_at("/frame", Accessible)
                .unwrap()
                .build(),
        ));
        let connection = connection.unwrap();
        let _server = server.unwrap();
        let root = AtspiElement::new(":1.1", "/frame", 1);
        let limits = WalkLimits::default();
        let mut reader =
            LevelReader::new(&connection, &root, &limits, Deadline::standard().unwrap());
        let root_read = reader
            .expand(&[root])
            .expect("the frame answers before transport loss");
        let child = root_read[0].children[0].clone();
        zbus::block_on(connection.clone().close()).unwrap();
        let error = reader
            .expand(&[child])
            .expect_err("a lost transport is not a vanished child");
        assert_eq!(error.code, ErrorCode::Internal);
    }
}
