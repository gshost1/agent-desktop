//! The overlays an action leaves open, reported beside its result.

use crate::adapter::PlatformAdapter;
use crate::{Deadline, ProcessIdentity, SurfaceInfo};
use std::time::Duration;

/// The longest an action spends reporting the surfaces it raised. A surface
/// inventory proves that no context menu is open by walking the whole
/// application tree; in an application with a large tree that walk outlasts
/// any action budget and its result is discarded. The action is already
/// delivered, so it must not wait out the rest of its own timeout here.
pub(crate) const SETTLED_SURFACE_BUDGET: Duration = Duration::from_millis(1_000);

/// An action that opens a sheet, menu, or alert leaves the caller staring at a
/// success envelope with no hint that the application is now waiting on a
/// dialog. Reporting the overlays saves a round of window archaeology. Purely
/// informational, so a failed or late read never disturbs delivered work.
pub(crate) fn read(
    adapter: &dyn PlatformAdapter,
    process: ProcessIdentity,
    deadline: Deadline,
) -> Vec<SurfaceInfo> {
    adapter
        .list_surfaces(process, deadline.capped(SETTLED_SURFACE_BUDGET))
        .unwrap_or_else(|error| {
            tracing::debug!(
                code = error.code.as_str(),
                "surface report after the action was dropped"
            );
            Vec::new()
        })
}

#[cfg(test)]
#[path = "settled_surface_tests.rs"]
mod tests;
