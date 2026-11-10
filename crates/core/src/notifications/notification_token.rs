use std::collections::HashMap;
use std::fmt;

use crate::NotificationInfo;

use super::notification_key::NotificationKey;

/// The opaque `key` that `list-notifications` prints for each entry: the
/// digest of the entry's app, title and body, its occurrence among the
/// entries identical to it, and how many such twins the list held. Every
/// adapter lists newest first, so the occurrence is counted from the bottom:
/// the oldest twin is 1, and an identical newcomer arriving at the top takes
/// the next number without renumbering the twins already listed. A twin that
/// leaves renumbers the newer ones, so a key whose twins have since dropped
/// below the recorded count cannot say which twin it named. Twins are grouped
/// by the full identity, never by the digest alone, so two different entries
/// whose digests collide carry the same key and are refused as ambiguous.
/// Keys are always assigned over the whole unfiltered list, before any filter
/// or limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NotificationToken {
    digest: u64,
    occurrence: usize,
    twins: usize,
}

impl NotificationToken {
    /// The key of every entry of `listed`, a newest-first list, in list
    /// order.
    pub(crate) fn assign<'a>(listed: impl IntoIterator<Item = &'a NotificationInfo>) -> Vec<Self> {
        let identities: Vec<NotificationKey> =
            listed.into_iter().map(NotificationKey::from).collect();
        Self::assign_by(&identities, NotificationKey::digest)
    }

    fn assign_by(
        identities: &[NotificationKey],
        digest: impl Fn(&NotificationKey) -> u64,
    ) -> Vec<Self> {
        let mut twins: HashMap<&NotificationKey, usize> = HashMap::new();
        for identity in identities {
            *twins.entry(identity).or_insert(0) += 1;
        }
        let mut seen: HashMap<&NotificationKey, usize> = HashMap::new();
        let mut keys: Vec<Self> = identities
            .iter()
            .rev()
            .map(|identity| {
                let occurrence = seen.entry(identity).or_insert(0);
                *occurrence += 1;
                Self {
                    digest: digest(identity),
                    occurrence: *occurrence,
                    twins: twins.get(identity).copied().unwrap_or(*occurrence),
                }
            })
            .collect();
        keys.reverse();
        keys
    }

    /// Whether `live`, a token from a fresh read, carries this key's digest
    /// and occurrence.
    pub(crate) fn names(&self, live: &Self) -> bool {
        self.digest == live.digest && self.occurrence == live.occurrence
    }

    /// Whether `live`, the fresh token this key names, has fewer twins than
    /// the list the key was printed from: one left, so the occurrence may now
    /// belong to another twin.
    pub(crate) fn lost_a_twin_by(&self, live: &Self) -> bool {
        live.twins < self.twins
    }

    /// Reads a key printed by [`fmt::Display`]; anything else is `None`.
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let (digest, counts) = text.split_once('-')?;
        if digest.len() != 16 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let (occurrence, twins) = counts.split_once("of")?;
        let occurrence = positive_count(occurrence)?;
        let twins = positive_count(twins).filter(|twins| *twins >= occurrence)?;
        Some(Self {
            digest: u64::from_str_radix(digest, 16).ok()?,
            occurrence,
            twins,
        })
    }
}

fn positive_count(text: &str) -> Option<usize> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok().filter(|count: &usize| *count > 0)
}

impl fmt::Display for NotificationToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:016x}-{}of{}",
            self.digest, self.occurrence, self.twins
        )
    }
}

/// The `key` of every entry of `listed`, a whole newest-first list, in list
/// order, as `list-notifications` prints it.
pub fn notification_keys(listed: &[NotificationInfo]) -> Vec<String> {
    NotificationToken::assign(listed)
        .iter()
        .map(ToString::to_string)
        .collect()
}

#[cfg(test)]
#[path = "notification_token_tests.rs"]
mod tests;
