use crate::{AdapterError, DeliverySemantics, ErrorCode, NotificationIdentity, NotificationInfo};

use super::notification_address::NotificationAddress;
use super::notification_filter::app_matches;
use super::notification_token::NotificationToken;

const RELIST: &str = "Run list-notifications again and retry with a freshly listed key";

/// Which notification a mutation acts on, named the way the caller saw it in
/// an earlier `list-notifications`. A key finds the entry wherever it now
/// sits; an index is only a position, so it is accepted only beside a key or
/// an expected app or title the entry there must still carry. Adapters
/// re-read the whole, unfiltered list under the mutation and act on the
/// entry [`Self::pick`] names; they never match or filter notifications
/// themselves.
#[derive(Debug, Clone)]
pub struct NotificationTarget {
    address: NotificationAddress,
    identity: NotificationIdentity,
    app: Option<String>,
}

impl NotificationTarget {
    /// Validates the caller's addressing: a key, or an index with at least
    /// one non-empty expected identity field. A bare index is refused
    /// because the list can shift between the two calls.
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
        Ok(Self {
            address,
            identity,
            app: None,
        })
    }

    /// The addressing check of [`Self::new`] alone, for callers that refuse
    /// a malformed request before they have an index to hand over.
    pub fn check(
        has_index: bool,
        key: Option<&str>,
        identity: &NotificationIdentity,
    ) -> Result<(), AdapterError> {
        Self::new(has_index.then_some(1), key, identity.clone()).map(|_| ())
    }

    /// Also requires the picked entry's app name to contain `app`, ignoring
    /// case, as `--app` does everywhere.
    pub fn with_app(mut self, app: Option<String>) -> Self {
        self.app = app.filter(|app| !app.is_empty());
        self
    }

    /// The position in `listed` of the one entry this target names. `listed`
    /// is the adapter's fresh, unfiltered read in list order. The entry is
    /// found by key or index first, then checked against the app scope and
    /// the expected fields. Every refusal is `not_delivered`.
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
        let info = listed[at];
        let app_ok = self.app.as_deref().is_none_or(|app| app_matches(info, app));
        if !app_ok || !self.identity.matches(info) {
            return Err(mismatched(info.index));
        }
        Ok(at)
    }
}

/// The refusal for an entry that is no longer the one the caller listed: the
/// key now names an entry at another index, or a re-read found another
/// object where the first read found the target.
pub fn notification_moved(index: usize) -> AdapterError {
    AdapterError::new(
        ErrorCode::NotificationNotFound,
        format!("The notification listed at index {index} moved or changed since it was listed"),
    )
    .with_suggestion(RELIST)
    .with_disposition(DeliverySemantics::not_delivered())
}

fn keyed(
    key: NotificationToken,
    index: Option<usize>,
    listed: &[&NotificationInfo],
) -> Result<usize, AdapterError> {
    let tokens = NotificationToken::assign(listed.iter().copied());
    let found: Vec<usize> = (0..listed.len())
        .filter(|at| key.names(&tokens[*at]))
        .collect();
    match (found.as_slice(), index) {
        ([at], _) if key.lost_a_twin_by(&tokens[*at]) => Err(twin_left(key)),
        ([at], None) => Ok(*at),
        ([at], Some(index)) if listed[*at].index == index => Ok(*at),
        ([_], Some(index)) => Err(notification_moved(index)),
        ([], _) => Err(gone(key)),
        _ => Err(AdapterError::new(
            ErrorCode::NotificationNotFound,
            format!("Notification key {key} names more than one listed notification"),
        )
        .with_suggestion(RELIST)
        .with_disposition(DeliverySemantics::not_delivered())),
    }
}

fn twin_left(key: NotificationToken) -> AdapterError {
    AdapterError::new(
        ErrorCode::NotificationNotFound,
        format!(
            "A notification identical to the one keyed {key} left since it was listed, so the key may now name a different twin"
        ),
    )
    .with_suggestion(RELIST)
    .with_disposition(DeliverySemantics::not_delivered())
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

fn mismatched(index: usize) -> AdapterError {
    AdapterError::new(
        ErrorCode::NotificationNotFound,
        format!("The notification at index {index} does not match the expected app or title"),
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
