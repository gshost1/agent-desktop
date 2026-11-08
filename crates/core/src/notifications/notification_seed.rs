use super::notification_baseline::NotificationBaseline;

/// What a batch read for a `wait --notification` before running the entry
/// that precedes it. Only `Captured` seeds the wait; the other two make the
/// wait take its own first list, and the wait result names why.
#[derive(Debug, Clone)]
pub enum NotificationSeed {
    Captured(NotificationBaseline),
    Unavailable,
    SkippedHeaded,
}

impl NotificationSeed {
    /// The baseline to start from, when the seed read succeeded.
    pub fn baseline(&self) -> Option<&NotificationBaseline> {
        match self {
            Self::Captured(baseline) => Some(baseline),
            Self::Unavailable | Self::SkippedHeaded => None,
        }
    }

    /// The `pre_action_seed` value a wait reports when it fell back to its
    /// own first list.
    pub fn fallback(&self) -> Option<&'static str> {
        match self {
            Self::Captured(_) => None,
            Self::Unavailable => Some("unavailable"),
            Self::SkippedHeaded => Some("skipped_headed"),
        }
    }
}
