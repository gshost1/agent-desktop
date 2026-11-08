use std::collections::HashMap;

use crate::NotificationInfo;

use super::notification_key::NotificationKey;

/// How many entries of each notification a `wait --notification` has already
/// seen. A later list matches when any notification has more entries than
/// recorded. A recorded count goes down only when two lists in a row agree on
/// the lower count, so a dismissed entry that arrives again identical is
/// reported as new, while one short or empty read cannot make an entry that
/// was there all along look new on the next full read.
#[derive(Debug, Clone)]
pub struct NotificationBaseline {
    counts: HashMap<NotificationKey, usize>,
    departures: HashMap<NotificationKey, usize>,
}

impl NotificationBaseline {
    /// Records every entry of `current` as already seen.
    pub fn from_list(current: &[NotificationInfo]) -> Self {
        Self {
            counts: counts(current),
            departures: HashMap::new(),
        }
    }

    /// Returns the first entry of `current` beyond the recorded count for its
    /// notification. Without one, a count lower than recorded is held as a
    /// pending departure and replaces the record when the next list agrees.
    pub(crate) fn observe<'a>(
        &mut self,
        current: &'a [NotificationInfo],
    ) -> Option<&'a NotificationInfo> {
        let listed = counts(current);
        if let Some(key) = current
            .iter()
            .map(NotificationKey::from)
            .find(|key| listed.get(key).copied().unwrap_or(0) > self.recorded(key))
        {
            let skip = self.recorded(&key);
            return current
                .iter()
                .filter(|info| NotificationKey::from(*info) == key)
                .nth(skip);
        }
        self.settle_departures(&listed);
        None
    }

    fn recorded(&self, key: &NotificationKey) -> usize {
        self.counts.get(key).copied().unwrap_or(0)
    }

    fn settle_departures(&mut self, listed: &HashMap<NotificationKey, usize>) {
        let previous = std::mem::take(&mut self.departures);
        let mut departures = HashMap::new();
        self.counts.retain(|key, count| {
            let now = listed.get(key).copied().unwrap_or(0);
            if now >= *count {
                return true;
            }
            if previous.get(key) == Some(&now) {
                *count = now;
                return now > 0;
            }
            departures.insert(key.clone(), now);
            true
        });
        self.departures = departures;
    }
}

fn counts(notifications: &[NotificationInfo]) -> HashMap<NotificationKey, usize> {
    let mut counts = HashMap::new();
    for notification in notifications {
        *counts
            .entry(NotificationKey::from(notification))
            .or_insert(0) += 1;
    }
    counts
}

#[cfg(test)]
#[path = "notification_baseline_tests.rs"]
mod tests;
