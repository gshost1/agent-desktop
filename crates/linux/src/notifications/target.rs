use agent_desktop_core::{
    AdapterError, Deadline, DeliverySemantics, ErrorCode, NotificationTarget,
};

use super::extract::Entry;
use super::list::{matches_app, read_listing};
use super::verify::same_identity;
use super::world::ShellWorld;
use crate::system::shell::light::{Light, same_object};

/// The entry a mutation aims at, as core names it, and the app scope the
/// caller asked for.
#[derive(Clone, Copy)]
pub(crate) struct Wanted<'a> {
    pub(crate) target: &'a NotificationTarget,
    pub(crate) app_filter: Option<&'a str>,
}

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

/// Reads the list and lets core pick the targeted entry from it. The app
/// filter is checked against the entry core picked, and a mismatch means the
/// list moved.
pub(crate) fn locate(
    world: &dyn ShellWorld,
    wanted: Wanted<'_>,
    deadline: Deadline,
) -> Result<Located, AdapterError> {
    let all = read_listing(world, deadline)?;
    let at = wanted.target.pick(all.iter().map(|entry| &entry.info))?;
    let info = &all[at].info;
    if !wanted.app_filter.is_none_or(|app| matches_app(info, app)) {
        return Err(moved(info.index));
    }
    Ok(Located { all, at })
}

pub(crate) fn moved(index: usize) -> AdapterError {
    AdapterError::new(
        ErrorCode::NotificationNotFound,
        format!("Notification at index {index} no longer matches its expected identity"),
    )
    .with_suggestion("Run list-notifications again and retry with the freshly observed index")
    .with_disposition(DeliverySemantics::not_delivered())
}

/// Reads again right before a press and refuses it when the entry core picks
/// is no longer the one the first read found: the same words on the same
/// accessibility object.
pub(crate) fn reconfirm(
    world: &dyn ShellWorld,
    wanted: Wanted<'_>,
    first: &Entry,
    deadline: Deadline,
) -> Result<Located, AdapterError> {
    let again = locate(world, wanted, deadline)?;
    let entry = again.entry();
    if !same_identity(&first.info, &entry.info) || !same_object(&first.node, &entry.node) {
        return Err(moved(first.info.index));
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
