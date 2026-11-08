use crate::{AdapterError, DeliverySemantics, ErrorCode, NotificationIdentity, NotificationInfo};

use super::notification_address::NotificationAddress;
use super::notification_token::NotificationToken;

const RELIST: &str = "Run list-notifications again and retry with a freshly listed key";

/// Which notification a mutation acts on, named the way the caller saw it in
/// an earlier `list-notifications`. A key finds the entry wherever it now
/// sits; an index is only a position, so it is accepted only beside a key or
/// an expected app or title the entry there must still carry. Adapters
/// re-read their list under the mutation and act on the entry [`Self::pick`]
/// names; they never match notifications themselves.
#[derive(Debug, Clone)]
pub struct NotificationTarget {
    address: NotificationAddress,
    identity: NotificationIdentity,
}

impl NotificationTarget {
    /// Validates the caller's addressing: a key, or an index with at least
    /// one expected identity field. A bare index is refused because the list
    /// can shift between the two calls.
    pub fn new(
        index: Option<usize>,
        key: Option<&str>,
        identity: NotificationIdentity,
    ) -> Result<Self, AdapterError> {
        let key = key
            .map(|text| NotificationToken::parse(text).ok_or_else(|| malformed(text)))
            .transpose()?;
        let address = match (key, index) {
            (Some(key), index) => NotificationAddress::Key { key, index },
            (None, Some(index)) if !identity.is_empty() => NotificationAddress::Index(index),
            (None, _) => return Err(unaddressed()),
        };
        Ok(Self { address, identity })
    }

    /// The position in `listed` of the one entry this target names. `listed`
    /// is the adapter's fresh read, in list order, unfiltered or filtered by
    /// app. A key that matches no entry, or an index whose entry no longer
    /// carries the key or the expected fields, is refused `not_delivered`.
    pub fn pick<'a>(
        &self,
        listed: impl IntoIterator<Item = &'a NotificationInfo>,
    ) -> Result<usize, AdapterError> {
        let listed: Vec<&NotificationInfo> = listed.into_iter().collect();
        let at = match self.address {
            NotificationAddress::Key { key, index } => keyed(key, index, &listed)?,
            NotificationAddress::Index(index) => listed
                .iter()
                .position(|info| info.index == index)
                .ok_or_else(|| AdapterError::notification_not_found(index))?,
        };
        if !self.identity.matches(listed[at]) {
            return Err(moved(listed[at].index));
        }
        Ok(at)
    }
}

fn keyed(
    key: NotificationToken,
    index: Option<usize>,
    listed: &[&NotificationInfo],
) -> Result<usize, AdapterError> {
    let tokens = NotificationToken::assign(listed.iter().copied());
    let found: Vec<usize> = (0..listed.len()).filter(|at| tokens[*at] == key).collect();
    match (found.as_slice(), index) {
        ([at], None) => Ok(*at),
        ([at], Some(index)) if listed[*at].index == index => Ok(*at),
        ([_], Some(index)) => Err(moved(index)),
        ([], _) => Err(gone(key)),
        _ => Err(AdapterError::new(
            ErrorCode::NotificationNotFound,
            format!("Notification key {key} names more than one listed notification"),
        )
        .with_suggestion(RELIST)
        .with_disposition(DeliverySemantics::not_delivered())),
    }
}

fn unaddressed() -> AdapterError {
    AdapterError::new(
        ErrorCode::InvalidArgs,
        "Notification mutations require --key, or an index with --expected-app or --expected-title",
    )
    .with_suggestion(
        "Pass the key from the same list-notifications entry so a reordered list cannot redirect the mutation",
    )
    .with_disposition(DeliverySemantics::not_delivered())
}

fn moved(index: usize) -> AdapterError {
    AdapterError::new(
        ErrorCode::NotificationNotFound,
        format!("Notification at index {index} no longer matches its expected identity"),
    )
    .with_suggestion(RELIST)
    .with_disposition(DeliverySemantics::not_delivered())
}

fn gone(key: NotificationToken) -> AdapterError {
    AdapterError::new(
        ErrorCode::NotificationNotFound,
        format!("No listed notification has key {key}"),
    )
    .with_suggestion(
        "The notification was dismissed, expired or changed; run list-notifications again",
    )
    .with_disposition(DeliverySemantics::not_delivered())
}

fn malformed(text: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::InvalidArgs,
        format!("'{text}' is not a notification key"),
    )
    .with_suggestion("Copy the key field of a list-notifications entry unchanged")
    .with_disposition(DeliverySemantics::not_delivered())
}

#[cfg(test)]
#[path = "notification_target_tests.rs"]
mod tests;
