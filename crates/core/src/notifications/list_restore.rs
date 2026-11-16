//! What a notification mutation could show about the surface it opened.

/// The state of the notification surface a mutation opened, when it could not
/// be shown put back. Serialized as `data.list_restore`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ListRestore {
    /// Closing the surface or restoring the previously frontmost app failed.
    Unverified,
}
