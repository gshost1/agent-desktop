use agent_desktop_core::{
    AdapterError, Deadline, DeliverySemantics, ErrorCode, NotificationTarget, notification_moved,
};

use super::extract::Entry;
use super::list::read_listing;
use super::verify::same_identity;
use super::world::ShellWorld;
use crate::system::shell::light::{Light, same_object};

/// A fresh read of the list with the entry a mutation targets in it.
pub(crate) struct Located {
    pub(crate) all: Vec<Entry>,
    at: usize,
}

impl Located {
    pub(crate) fn entry(&self) -> &Entry {
        &self.all[self.at]
    }
}

/// Reads the whole list and lets core pick the targeted entry from it.
pub(crate) fn locate(
    world: &dyn ShellWorld,
    target: &NotificationTarget,
    deadline: Deadline,
) -> Result<Located, AdapterError> {
    let all = read_listing(world, deadline)?;
    let at = target.pick(all.iter().map(|entry| &entry.info))?;
    Ok(Located { all, at })
}

/// Reads again right before a press and refuses it when the entry core picks
/// is no longer the one the first read found: the same words on the same
/// accessibility object.
pub(crate) fn reconfirm(
    world: &dyn ShellWorld,
    target: &NotificationTarget,
    first: &Entry,
    deadline: Deadline,
) -> Result<Located, AdapterError> {
    let again = locate(world, target, deadline)?;
    let entry = again.entry();
    if !same_identity(&first.info, &entry.info) || !same_object(&first.node, &entry.node) {
        return Err(notification_moved(first.info.index));
    }
    Ok(again)
}

pub(crate) fn missing_control(what: &str, index: usize) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        format!("The notification at index {index} shows no {what} to click"),
    )
    .with_suggestion("Run list-notifications and retry with a freshly observed entry")
    .with_disposition(DeliverySemantics::not_delivered())
}

pub(crate) fn require(
    control: Option<&Light>,
    what: &str,
    index: usize,
) -> Result<Light, AdapterError> {
    control.cloned().ok_or_else(|| missing_control(what, index))
}

#[cfg(test)]
#[path = "target_tests.rs"]
mod tests;
