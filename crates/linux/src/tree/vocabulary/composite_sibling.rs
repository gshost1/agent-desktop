use futures_util::future::{join, try_join};
use zbus::Connection;

use super::composite_label::accessible;
use super::role_numbers::COMBO_BOX;
use super::state_bits::{EDITABLE, StateBits};
use crate::tree::AtspiElement;
use crate::tree::element::bounded::collect_bounded;
use crate::tree::properties::absence::failure;
use crate::tree::properties::failure::Failure;
use crate::tree::properties::liveness::Liveness;

pub(super) const EDITABLE_TEXT: &str = "org.a11y.atspi.EditableText";
pub(super) const CHILDREN_IN_FLIGHT: usize = 8;

/// Reads the field's siblings a few at a time and stops at the first editable
/// one, which no field can share a label with. A sibling whose own `GetRole`
/// shows it destroyed is absent.
pub(super) async fn read_children(
    connection: &Connection,
    siblings: &[(usize, AtspiElement)],
    known_editable: usize,
    need_role: bool,
) -> Result<Vec<(usize, bool, bool)>, zbus::Error> {
    let mut reads = Vec::with_capacity(siblings.len());
    for chunk in siblings.chunks(CHILDREN_IN_FLIGHT) {
        let chunk_reads = collect_bounded(
            CHILDREN_IN_FLIGHT,
            chunk
                .iter()
                .map(|(_, sibling)| read_child(connection, sibling, need_role)),
        )
        .await;
        for ((index, _), read) in chunk.iter().zip(chunk_reads) {
            match read {
                Ok(Some((editable, combo))) => reads.push((*index, editable, combo)),
                Ok(None) => {}
                Err(error) => return Err(error),
            }
        }
        if known_editable + reads.iter().filter(|read| read.1).count() > 1 {
            break;
        }
    }
    Ok(reads)
}

async fn read_child(
    connection: &Connection,
    child: &AtspiElement,
    need_role: bool,
) -> Result<Option<(bool, bool)>, zbus::Error> {
    let read_role = || accessible::<u32>(connection, child, "GetRole");
    let (editability, role) = join(
        try_join(
            accessible::<Vec<u32>>(connection, child, "GetState"),
            accessible::<Vec<String>>(connection, child, "GetInterfaces"),
        ),
        async {
            if need_role {
                Some(read_role().await)
            } else {
                None
            }
        },
    )
    .await;
    let role = match role {
        None if editability
            .as_ref()
            .is_err_and(|error| failure(error) == Failure::Suspect) =>
        {
            Some(read_role().await)
        }
        role => role,
    };
    if Liveness::of(role.as_ref().and_then(|role| role.as_ref().err())) == Liveness::ObjectGone {
        return Ok(None);
    }
    let (states, interfaces) = editability?;
    let editable = StateBits::new(&states).has(EDITABLE)
        || interfaces.iter().any(|name| name == EDITABLE_TEXT);
    Ok(Some((editable, role.transpose()? == Some(COMBO_BOX))))
}
