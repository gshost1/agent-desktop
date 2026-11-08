use std::collections::HashMap;

use crate::NotificationInfo;

use super::notification_key::NotificationKey;

/// How many entries of each notification a `wait --notification` has already
/// seen. A later list matches when any notification has more entries than
/// recorded. A list that matches nothing becomes the new record, so an entry
/// that is dismissed lowers its count and an identical one that arrives later
/// is reported as new.
#[derive(Debug, Clone, Default)]
pub struct NotificationBaseline {
    counts: HashMap<NotificationKey, usize>,
}

impl NotificationBaseline {
    /// Records every entry of `current` as already seen.
    pub fn from_list(current: &[NotificationInfo]) -> Self {
        Self {
            counts: counts(current),
        }
    }

    /// Returns the first entry of `current` beyond the recorded count for its
    /// notification. Without one, `current` replaces the record.
    pub(crate) fn observe<'a>(
        &mut self,
        current: &'a [NotificationInfo],
    ) -> Option<&'a NotificationInfo> {
        let mut seen: HashMap<NotificationKey, usize> = HashMap::new();
        for notification in current {
            let key = NotificationKey::from(notification);
            let recorded = self.counts.get(&key).copied().unwrap_or(0);
            let count = seen.entry(key).or_insert(0);
            *count += 1;
            if *count > recorded {
                return Some(notification);
            }
        }
        self.counts = seen;
        None
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
