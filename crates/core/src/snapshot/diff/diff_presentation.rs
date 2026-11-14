use serde::{Deserialize, Serialize};

use crate::{SnapshotSurface, adapter::TreeOptions};

/// The tree-shaping options a snapshot was presented with. A diff between two
/// differently shaped trees would report the shaping as change, so the two
/// snapshots must agree on all of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DiffPresentation {
    pub(crate) surface: SnapshotSurface,
    pub(crate) max_depth: u8,
    pub(crate) interactive_only: bool,
    pub(crate) compact: bool,
    pub(crate) skeleton: bool,
}

impl DiffPresentation {
    pub(crate) fn of(opts: &TreeOptions) -> Self {
        Self {
            surface: opts.surface,
            max_depth: opts.max_depth,
            interactive_only: opts.interactive_only,
            compact: opts.compact,
            skeleton: opts.skeleton,
        }
    }
}
