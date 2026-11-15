//! The reads a followed post-action wait makes of its process's windows and
//! overlays, and which failed reads it can pass over.
//!
//! A window that closed between the listing and the read, or a surface a
//! complete read proved absent, shows nothing. Any other failed read leaves
//! the poll incomplete, except a failure that means the application cannot
//! be read at all: a denied permission or an unresponsive owner. That one is
//! kept while the rest of the poll's windows and overlays are read, and is
//! the poll's answer only when none of them matched, so it reaches the
//! unresponsive check and the caller with its own code and is never taken
//! for a closed window.

use crate::{
    AdapterError, AppError, Deadline, ErrorCode, LocatorQuery, PlatformAdapter, RefEntry,
    SnapshotOwner, SnapshotSurface, WindowInfo,
    commands::selector_observation::SelectorObservation,
    system::window_lookup::surface_owner_order,
    wait::window_baseline::{WINDOW_BASELINE_BUDGET, process_windows},
};

use super::{Matched, Observed};

const OVERLAY_SURFACES: [SnapshotSurface; 3] = [
    SnapshotSurface::Sheet,
    SnapshotSurface::Alert,
    SnapshotSurface::Popover,
];

/// What the reads of one poll found short of a match, across its windows and
/// overlays: whether every read was complete, and the first failure that
/// means the application cannot be read.
pub(super) struct Unmatched {
    complete: bool,
    unreadable: Option<AppError>,
}

impl Unmatched {
    pub(super) fn new() -> Self {
        Self {
            complete: true,
            unreadable: None,
        }
    }

    pub(super) fn record_read(&mut self, complete: bool) {
        self.complete &= complete;
    }

    /// Keeps an unreadable application's error for the end of the poll and
    /// returns any other error, for a read whose failure ends the wait.
    pub(super) fn keep_unreadable(&mut self, error: AppError) -> Result<(), AppError> {
        if !is_unreadable(&error) {
            return Err(error);
        }
        self.unreadable.get_or_insert(error);
        Ok(())
    }

    pub(super) fn record_error(&mut self, error: AppError) {
        if shows_nothing(&error) {
            return;
        }
        if is_unreadable(&error) {
            self.unreadable.get_or_insert(error);
        } else {
            self.complete = false;
        }
    }

    /// The windows a listing named. A listing that failed leaves the scope
    /// incomplete: the reads of the windows it does name are what tell an
    /// unreadable application apart.
    pub(super) fn listed(
        &mut self,
        listing: Result<Vec<WindowInfo>, AdapterError>,
    ) -> Vec<WindowInfo> {
        listing.unwrap_or_else(|_| {
            self.complete = false;
            Vec::new()
        })
    }

    /// The poll's answer once nothing matched: the unreadable application
    /// first, then a complete or an incomplete miss.
    pub(super) fn finish(self) -> Observed {
        match self.unreadable {
            Some(error) => Err(error),
            None => Ok(self.complete.then_some((false, None))),
        }
    }
}

/// `(matched, complete)` for one owner; a match counts only from a complete
/// read.
pub(super) fn observe_owner(
    adapter: &dyn PlatformAdapter,
    owner: &SnapshotOwner,
    query: &LocatorQuery,
    deadline: Deadline,
) -> Result<(bool, bool), AppError> {
    let observed = SelectorObservation::observe_owners(
        adapter,
        vec![owner.clone()],
        SnapshotSurface::Window,
        query,
        deadline,
    )?;
    Ok(match observed {
        SelectorObservation::Selector { matches, complete } => (matches && complete, complete),
        SelectorObservation::OwnerUnavailable(_) => (false, false),
    })
}

/// Reads each window in turn and returns the first that holds the selector.
pub(super) fn observe_each(
    adapter: &dyn PlatformAdapter,
    windows: Vec<WindowInfo>,
    query: &LocatorQuery,
    deadline: Deadline,
    unmatched: &mut Unmatched,
) -> Option<Matched> {
    for window in windows {
        let owner = SnapshotOwner::Window(window);
        match observe_owner(adapter, &owner, query, deadline) {
            Ok((true, _)) => return Some(Some((owner, SnapshotSurface::Window))),
            Ok((false, complete)) => unmatched.record_read(complete),
            Err(error) => unmatched.record_error(error),
        }
    }
    None
}

/// The sheet, alert or popover of the ref's own process instance that holds
/// the selector, with the window it is attached to. Each window is read on
/// its own, so an overlay without the selector on one window never hides one
/// with it on another.
pub(super) fn read_overlays(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    query: &LocatorQuery,
    deadline: Deadline,
    unmatched: &mut Unmatched,
) -> Option<(SnapshotOwner, SnapshotSurface)> {
    let (Some(app), Some(process)) = (entry.source.source_app.as_deref(), entry.process.identity())
    else {
        return None;
    };
    let surfaces: Vec<_> = OVERLAY_SURFACES
        .into_iter()
        .filter(|surface| adapter.check_surface_support(*surface).is_ok())
        .collect();
    if surfaces.is_empty() {
        return None;
    }
    let mut windows = unmatched.listed(process_windows(
        adapter,
        app,
        &process,
        deadline.capped(WINDOW_BASELINE_BUDGET),
    ));
    windows.retain(|window| window.state.accessible);
    let owners: Vec<_> = surface_owner_order(windows)
        .into_iter()
        .map(SnapshotOwner::Window)
        .collect();
    for surface in surfaces {
        for owner in &owners {
            match SelectorObservation::observe_owners(
                adapter,
                vec![owner.clone()],
                surface,
                query,
                deadline,
            ) {
                Ok(SelectorObservation::Selector {
                    matches: true,
                    complete: true,
                }) => return Some((owner.clone(), surface)),
                Ok(_) => {}
                Err(error) => unmatched.record_error(error),
            }
        }
    }
    None
}

/// A read that found its window closed, or a surface proved absent.
fn shows_nothing(error: &AppError) -> bool {
    matches!(
        error,
        AppError::Adapter(error)
            if matches!(error.code, ErrorCode::WindowNotFound | ErrorCode::ElementNotFound)
                || error.details.as_ref().is_some_and(|details| {
                    details["kind"] == "surface_absent" || details["kind"] == "menu_closed"
                })
    )
}

/// A read that failed because the application cannot be read at all.
fn is_unreadable(error: &AppError) -> bool {
    matches!(
        error,
        AppError::Adapter(error)
            if matches!(error.code, ErrorCode::PermDenied | ErrorCode::AppUnresponsive)
    )
}
