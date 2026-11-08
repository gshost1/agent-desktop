use std::collections::HashMap;
use std::fmt;

use crate::NotificationInfo;

use super::notification_key::NotificationKey;

/// The opaque `key` that `list-notifications` prints for each entry: the
/// digest of the entry's app, title and body, and its occurrence among the
/// entries with that digest. Every adapter lists newest first, so the
/// occurrence is counted from the bottom: the oldest twin is 1, and an
/// identical newcomer arriving at the top takes the next number without
/// renumbering the twins already listed. Keys are always assigned over the
/// whole unfiltered list, before any filter or limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NotificationToken {
    digest: u64,
    occurrence: usize,
}

impl NotificationToken {
    /// The key of every entry of `listed`, a newest-first list, in list
    /// order.
    pub(crate) fn assign<'a>(listed: impl IntoIterator<Item = &'a NotificationInfo>) -> Vec<Self> {
        let digests: Vec<u64> = listed
            .into_iter()
            .map(|info| NotificationKey::from(info).digest())
            .collect();
        let mut seen: HashMap<u64, usize> = HashMap::new();
        let mut keys: Vec<Self> = digests
            .iter()
            .rev()
            .map(|digest| {
                let occurrence = seen.entry(*digest).or_insert(0);
                *occurrence += 1;
                Self {
                    digest: *digest,
                    occurrence: *occurrence,
                }
            })
            .collect();
        keys.reverse();
        keys
    }

    /// Reads a key printed by [`fmt::Display`]; anything else is `None`.
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let (digest, occurrence) = text.split_once('-')?;
        if digest.len() != 16 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        if !occurrence.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        let occurrence = occurrence.parse().ok().filter(|count: &usize| *count > 0)?;
        Some(Self {
            digest: u64::from_str_radix(digest, 16).ok()?,
            occurrence,
        })
    }
}

impl fmt::Display for NotificationToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:016x}-{}", self.digest, self.occurrence)
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
