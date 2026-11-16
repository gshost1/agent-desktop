use crate::{SnapshotOwner, SnapshotSurface};

/// Where a wait's read found its selector, and how far below that read's root
/// the first match sits, so the snapshot the wait returns can reach it.
pub(crate) struct SelectorHit {
    /// The owner and surface the read matched on, or `None` for the target
    /// the wait was given, such as the ref's own application surface.
    pub(crate) owner: Option<(SnapshotOwner, SnapshotSurface)>,
    pub(crate) depth: u8,
}

impl SelectorHit {
    pub(crate) fn on_target(depth: u8) -> Self {
        Self { owner: None, depth }
    }

    pub(crate) fn on(owner: SnapshotOwner, surface: SnapshotSurface, depth: u8) -> Self {
        Self {
            owner: Some((owner, surface)),
            depth,
        }
    }
}
