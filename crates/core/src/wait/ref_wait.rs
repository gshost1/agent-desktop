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

/// Whether a ref is owned by one window, so following it can reach other
/// windows. Refs owned by an application surface or a process root already
/// observe their whole owner.
pub(crate) fn follows_windows(adapter: &dyn PlatformAdapter, entry: &RefEntry) -> bool {
    !adapter.is_application_surface(entry.source.source_surface) && !entry.source.is_process_owned()
}
