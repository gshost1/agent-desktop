//! The ref a post-action wait follows, with the windows its process had open
//! before the action.

use crate::{PlatformAdapter, RefEntry, wait::window_baseline::WindowBaseline};

/// A ref the wait follows, with the windows its process had open before the
/// action when those could be listed.
#[derive(Clone, Copy)]
pub(crate) struct RefWait<'a> {
    pub(crate) entry: &'a RefEntry,
    pub(crate) baseline: Option<&'a WindowBaseline>,
}

impl RefWait<'_> {
    /// Whether the wait follows this ref past its window: only an appear
    /// wait does, and only for a window-owned ref.
    pub(crate) fn follows(&self, adapter: &dyn PlatformAdapter, gone: bool) -> bool {
        !gone && follows_windows(adapter, self.entry)
    }
}

/// Whether a ref is observed through one window's native tree, so following
/// it can reach other windows. Refs owned by an application surface or a
/// process root already observe their whole owner, and a web ref is observed
/// on its page.
pub(crate) fn follows_windows(adapter: &dyn PlatformAdapter, entry: &RefEntry) -> bool {
    !entry.source.is_process_owned()
        && crate::snapshot::surface_target::ref_observation_surface(adapter, entry)
            == crate::SnapshotSurface::Window
}
