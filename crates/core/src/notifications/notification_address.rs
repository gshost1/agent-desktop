use super::notification_token::NotificationToken;

/// How a mutation's target is addressed: by the key `list-notifications`
/// printed, optionally pinned to the index it was printed at, or by an index
/// alone, which [`super::notification_target::NotificationTarget`] only
/// accepts beside an expected app or title.
#[derive(Debug, Clone, Copy)]
pub(crate) enum NotificationAddress {
    Key {
        key: NotificationToken,
        index: Option<usize>,
    },
    Index(usize),
}
