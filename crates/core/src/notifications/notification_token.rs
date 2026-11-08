use std::collections::HashMap;
use std::fmt;

use crate::NotificationInfo;

use super::notification_key::NotificationKey;

/// The opaque `key` that `list-notifications` prints for each entry: the
/// digest of the entry's app, title and body, and its occurrence among the
/// entries with that digest, counted from the top of the list. The
/// occurrence tells identical twins apart. List filters only drop whole
/// groups of twins or cut the list from the bottom, so a filtered list and a
/// full list assign every shown entry the same key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NotificationToken {
    digest: u64,
    occurrence: usize,
}

impl NotificationToken {
    /// The key of every entry of `listed`, in list order.
    pub(crate) fn assign<'a>(listed: impl IntoIterator<Item = &'a NotificationInfo>) -> Vec<Self> {
        let mut seen: HashMap<u64, usize> = HashMap::new();
        listed
            .into_iter()
            .map(|info| {
                let digest = NotificationKey::from(info).digest();
                let occurrence = seen.entry(digest).or_insert(0);
                *occurrence += 1;
                Self {
                    digest,
                    occurrence: *occurrence,
                }
            })
            .collect()
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

/// The `key` of every entry of `listed`, in list order, as
/// `list-notifications` prints it.
pub fn notification_keys(listed: &[NotificationInfo]) -> Vec<String> {
    NotificationToken::assign(listed)
        .iter()
        .map(ToString::to_string)
        .collect()
}

#[cfg(test)]
#[path = "notification_token_tests.rs"]
mod tests;
