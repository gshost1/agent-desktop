/// What a snapshot reports beside its tree: why its tokenless refs are
/// unproven, if they are, and which channel read it, if the surface names one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SnapshotNotes {
    /// Set when the whole-window read that proves tokenless refs failed or was
    /// cut short; it says why, and those refs are refused when used.
    pub identity_unproven: Option<String>,
    pub channel: Option<crate::SnapshotChannel>,
}
