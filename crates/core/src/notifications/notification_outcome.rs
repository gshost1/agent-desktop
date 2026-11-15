//! A notification mutation's result beside what its cleanup could show.

use crate::ListRestore;

/// The result of a notification mutation and whether the surface it opened
/// was put back afterwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationOutcome<T> {
    pub value: T,
    /// `Some` when the mutation was delivered but closing the surface it
    /// opened, or restoring the app it took focus from, could not be shown.
    pub list_restore: Option<ListRestore>,
}

impl<T> From<T> for NotificationOutcome<T> {
    fn from(value: T) -> Self {
        Self {
            value,
            list_restore: None,
        }
    }
}

impl<T> NotificationOutcome<T> {
    /// A delivered result whose cleanup could not be shown done.
    pub fn with_unverified_restore(value: T) -> Self {
        Self {
            value,
            list_restore: Some(ListRestore::Unverified),
        }
    }
}

/// What a dismiss-all sweep removed and which entries it could not close.
pub type DismissAllOutcome = NotificationOutcome<(Vec<crate::NotificationInfo>, Vec<String>)>;
