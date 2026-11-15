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
    /// Whether the wait follows this ref past its own scope: only an appear
    /// wait does, and only for a ref whose read does not already cover every
    /// window of its process.
    pub(crate) fn follows(&self, adapter: &dyn PlatformAdapter, gone: bool) -> bool {
        !gone && follows_windows(adapter, self.entry)
    }
}

/// Whether following a ref can reach windows its own read does not: a ref
/// observed through one window's native tree, or one on an application
/// surface such as a menu bar item, whose read covers only that surface. A
/// process root already observes its whole process, and a web ref is
/// observed on its page.
pub(crate) fn follows_windows(adapter: &dyn PlatformAdapter, entry: &RefEntry) -> bool {
    if adapter.is_application_surface(entry.source.source_surface) {
        return true;
    }
    !entry.source.is_process_owned()
        && crate::snapshot::surface_target::ref_observation_surface(adapter, entry)
            == crate::SnapshotSurface::Window
}
