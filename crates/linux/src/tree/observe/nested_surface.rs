//! Finds the dialog a sheet or alert request names when it is drawn inside the
//! window instead of being a window of its own.

use agent_desktop_core::{
    AdapterError, ErrorCode, ObservationRequest, ObservationRoot, SnapshotSurface,
};

use super::{SettleHost, full_read, window_gone};
use crate::tree::properties::read_purpose::ReadPurpose;
use crate::tree::surfaces::is_surface;
use crate::tree::surfaces::nested_dialog::{
    find_surface as find_nested, is_incomplete_surface as is_incomplete,
};
use crate::tree::{AtspiElement, ObservedRead, Walk, WalkLimits};

fn dialog_absent(surface: SnapshotSurface) -> AdapterError {
    AdapterError::new(
        ErrorCode::ElementNotFound,
        format!("The window does not present a {} surface", surface.as_str()),
    )
    .with_suggestion("Open a dialog of the requested kind, or snapshot the window surface")
}

/// The limits of the search for a dialog. The request's depth limit bounds the
/// surface and not the search for it, because a dialog may sit below that cut.
fn discovery_limits(limits: &WalkLimits) -> WalkLimits {
    let whole = WalkLimits::default();
    WalkLimits {
        max_raw_depth: whole.max_raw_depth,
        max_logical_depth: whole.max_logical_depth,
        ..*limits
    }
}

/// For a sheet or alert request on a window, the walk of the dialog drawn
/// inside that window, rooted at the dialog so its refs resolve against it.
/// `None` means the window itself is the requested surface and is observed as
/// usual. Any other request is `None` without a read.
///
/// The search is a structural read that stops at web documents: a nested
/// dialog is never looked for inside page content, so a browser window costs
/// its own frame and not its page. Only a found dialog gets the full read. A
/// search that could not read the whole window proves neither absence nor
/// uniqueness and is a timeout.
pub(super) fn discover(
    root: &ObservationRoot<'_>,
    window: &AtspiElement,
    limits: &WalkLimits,
    request: &ObservationRequest,
    host: &mut SettleHost<'_>,
) -> Result<Option<(Walk<ObservedRead>, bool)>, AdapterError> {
    let surface = request.surface;
    if !matches!(surface, SnapshotSurface::Sheet | SnapshotSurface::Alert)
        || !matches!(root, ObservationRoot::Window(_))
    {
        return Ok(None);
    }
    let scan = (host.walker)(
        window,
        &discovery_limits(limits),
        request.deadline,
        ReadPurpose::Structure,
    )
    .map_err(|error| window_gone(root, error))?;
    if is_surface(&scan.root.payload.node, surface) {
        return Ok(None);
    }
    if !scan.complete || is_incomplete(&scan.root, surface) {
        return Err(AdapterError::timeout(
            "Dialog discovery did not read the complete window",
        ));
    }
    let dialog = match find_nested(&scan.root, surface).as_slice() {
        [dialog] => dialog.element.clone(),
        [] => return Err(dialog_absent(surface)),
        _ => {
            return Err(AdapterError::new(
                ErrorCode::AmbiguousTarget,
                format!("The window presents multiple {} surfaces", surface.as_str()),
            )
            .with_suggestion("Snapshot the window and select a dialog by ref"));
        }
    };
    let walk = (host.walker)(&dialog, limits, request.deadline, full_read(request))?;
    Ok(Some((walk, true)))
}

#[cfg(test)]
#[path = "nested_surface_tests.rs"]
mod tests;
