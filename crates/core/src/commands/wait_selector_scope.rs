//! What a post-action `--wait-for` observes for a window-owned ref, and the
//! outcomes that end it before its budget.
//!
//! An action often shows its effect outside the window it was sent to: a
//! button opens a dialog, a menu item opens a new window, an OK button closes
//! its own dialog. The wait therefore reads the ref's window first, then the
//! windows its process opened since the action, and every window of the
//! process once the ref's window has closed. A process that exited or crashed
//! can show nothing more, so the wait ends at once instead of at its budget.

use serde_json::json;

use crate::{
    AdapterError, AppError, Deadline, ErrorCode, LocatorQuery, PlatformAdapter, RefEntry,
    SnapshotOwner, SnapshotSurface,
    commands::selector_observation::SelectorObservation,
    snapshot::{ref_owner_observation::RefOwnerObservation, surface_target},
    system::process_state::ProcessState,
    wait::window_baseline::{WindowBaseline, process_windows},
};

const TARGET_GONE: &str = "wait_target_gone";
const OWNER_UNRESPONSIVE: &str = "wait_owner_unresponsive";

/// A ref the wait follows, with the windows its process had open before the
/// action when those could be listed.
#[derive(Clone, Copy)]
pub(crate) struct RefWait<'a> {
    pub(crate) entry: &'a RefEntry,
    pub(crate) baseline: Option<&'a WindowBaseline>,
}

impl RefWait<'_> {
    /// Whether an appear wait follows this ref past its own scope: a ref owned
    /// by a process root already observes the whole process, and a gone wait
    /// keeps the scope the selector was in.
    pub(crate) fn follows(&self, adapter: &dyn PlatformAdapter, gone: bool) -> bool {
        !gone && !crate::wait::window_baseline::observes_whole_process(adapter, self.entry)
    }

    /// Whether the ref was captured from a surface the application owns, such
    /// as a macOS menu bar or menu. Its command acts in a window, so the wait
    /// reads the windows after the surface itself.
    fn captured_on_application_surface(&self, adapter: &dyn PlatformAdapter) -> bool {
        adapter.is_application_surface(self.entry.source.source_surface)
    }
}

/// One read of a followed scope.
pub(crate) enum ScopeObservation {
    Matched(SnapshotOwner, SnapshotSurface),
    MatchedOwnSurface,
    Unmatched { complete: bool },
    ProcessGone(ProcessState),
    Unresponsive(ProcessState),
}

pub(crate) fn observe(
    adapter: &dyn PlatformAdapter,
    target: RefWait<'_>,
    query: &LocatorQuery,
    deadline: Deadline,
) -> Result<ScopeObservation, AppError> {
    if target.captured_on_application_surface(adapter) {
        return observe_application_ref(adapter, target, query, deadline);
    }
    let (owners, ref_window_open) =
        match surface_target::resolve_ref_owner(adapter, target.entry, deadline)? {
            RefOwnerObservation::Present(owner) => {
                let mut owners = vec![owner];
                owners.extend(opened_windows(adapter, target.baseline, deadline));
                (owners, true)
            }
            RefOwnerObservation::Absent(None) => {
                match every_window(adapter, target.entry, deadline) {
                    Some(owners) => (owners, false),
                    None => return Ok(ScopeObservation::Unmatched { complete: false }),
                }
            }
            RefOwnerObservation::Absent(Some(state)) => {
                return Ok(ScopeObservation::ProcessGone(state));
            }
            RefOwnerObservation::Unavailable(state) => {
                return Ok(ScopeObservation::Unresponsive(state));
            }
        };
    let observed = observe_windows(adapter, owners, ref_window_open, query, deadline)?;
    Ok(or_overlays(adapter, target, observed, query, deadline))
}

/// Reads the ref's own application surface, then the windows its command
/// acts in. A menu item's effect, such as the sheet it opens, never shows in
/// the menu bar the ref came from.
fn observe_application_ref(
    adapter: &dyn PlatformAdapter,
    target: RefWait<'_>,
    query: &LocatorQuery,
    deadline: Deadline,
) -> Result<ScopeObservation, AppError> {
    let own_complete =
        match SelectorObservation::observe_ref(adapter, target.entry, query, deadline) {
            Ok(SelectorObservation::Selector {
                matches: true,
                complete: true,
            }) => return Ok(ScopeObservation::MatchedOwnSurface),
            Ok(SelectorObservation::Selector { complete, .. }) => complete,
            Ok(SelectorObservation::OwnerUnavailable(
                state @ (ProcessState::Exited { .. } | ProcessState::Crashed { .. }),
            )) => return Ok(ScopeObservation::ProcessGone(state)),
            Ok(SelectorObservation::OwnerUnavailable(state)) => {
                return Ok(ScopeObservation::Unresponsive(state));
            }
            Err(error) => closed_meanwhile(&error) || is_surface_absent(&error),
        };
    let windows = target
        .baseline
        .and_then(|baseline| baseline.acted_on(adapter, deadline).ok())
        .unwrap_or_default()
        .into_iter()
        .map(SnapshotOwner::Window)
        .collect();
    let observed = observe_windows(adapter, windows, false, query, deadline)?;
    Ok(
        match or_overlays(adapter, target, observed, query, deadline) {
            ScopeObservation::Unmatched { complete } => ScopeObservation::Unmatched {
                complete: complete && own_complete,
            },
            other => other,
        },
    )
}

const OVERLAY_SURFACES: [SnapshotSurface; 3] = [
    SnapshotSurface::Sheet,
    SnapshotSurface::Alert,
    SnapshotSurface::Popover,
];

/// A sheet, alert or popover the action opened is not part of its window's
/// tree, so an unmatched read of the windows goes on to the overlays the
/// application has open.
fn or_overlays(
    adapter: &dyn PlatformAdapter,
    target: RefWait<'_>,
    observed: ScopeObservation,
    query: &LocatorQuery,
    deadline: Deadline,
) -> ScopeObservation {
    let ScopeObservation::Unmatched { complete } = observed else {
        return observed;
    };
    let Some(app) = target.entry.source.source_app.as_deref() else {
        return observed;
    };
    for surface in OVERLAY_SURFACES {
        if adapter.check_surface_support(surface).is_err() {
            continue;
        }
        let Ok(owners) =
            surface_target::resolve_owners(adapter, Some(app), None, surface, deadline)
        else {
            continue;
        };
        let Some(owner) = owners.first().cloned() else {
            continue;
        };
        if let Ok(SelectorObservation::Selector {
            matches: true,
            complete: true,
        }) = SelectorObservation::observe_owners(adapter, owners, surface, query, deadline)
        {
            return ScopeObservation::Matched(owner, surface);
        }
    }
    ScopeObservation::Unmatched { complete }
}

fn is_surface_absent(error: &AppError) -> bool {
    matches!(
        error,
        AppError::Adapter(error) if error.details.as_ref().is_some_and(|details| {
            details["kind"] == "surface_absent" || details["kind"] == "menu_closed"
        })
    )
}

fn observe_windows(
    adapter: &dyn PlatformAdapter,
    owners: Vec<SnapshotOwner>,
    ref_window_open: bool,
    query: &LocatorQuery,
    deadline: Deadline,
) -> Result<ScopeObservation, AppError> {
    let mut complete = true;
    for (index, owner) in owners.into_iter().enumerate() {
        let observed = SelectorObservation::observe_owners(
            adapter,
            vec![owner.clone()],
            SnapshotSurface::Window,
            query,
            deadline,
        );
        match observed {
            Ok(SelectorObservation::Selector {
                matches: true,
                complete: true,
            }) => return Ok(ScopeObservation::Matched(owner, SnapshotSurface::Window)),
            Ok(SelectorObservation::Selector { complete: read, .. }) => complete &= read,
            Ok(SelectorObservation::OwnerUnavailable(_)) => complete = false,
            Err(error) if index == 0 && ref_window_open => return Err(error),
            Err(error) => complete &= closed_meanwhile(&error),
        }
    }
    Ok(ScopeObservation::Unmatched { complete })
}

fn opened_windows(
    adapter: &dyn PlatformAdapter,
    baseline: Option<&WindowBaseline>,
    deadline: Deadline,
) -> Vec<SnapshotOwner> {
    baseline
        .and_then(|baseline| baseline.opened_since(adapter, deadline).ok())
        .unwrap_or_default()
        .into_iter()
        .map(SnapshotOwner::Window)
        .collect()
}

fn every_window(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    deadline: Deadline,
) -> Option<Vec<SnapshotOwner>> {
    let app = entry.source.source_app.as_deref()?;
    let process = entry.process.identity()?;
    let windows = process_windows(adapter, app, &process, deadline).ok()?;
    Some(windows.into_iter().map(SnapshotOwner::Window).collect())
}

fn closed_meanwhile(error: &AppError) -> bool {
    matches!(
        error,
        AppError::Adapter(error)
            if matches!(error.code, ErrorCode::WindowNotFound | ErrorCode::ElementNotFound)
    )
}

pub(crate) type Observed =
    Result<Option<(bool, Option<(SnapshotOwner, SnapshotSurface)>)>, AppError>;

/// Reads a followed scope. A gone process is decided at once; an unresponsive
/// one is a retryable read the caller times by its spell.
pub(crate) fn observe_followed(
    adapter: &dyn PlatformAdapter,
    target: RefWait<'_>,
    (query, query_raw): (&crate::LocatorQuery, &str),
    deadline: crate::Deadline,
) -> Observed {
    match observe(adapter, target, query, deadline)? {
        ScopeObservation::Matched(owner, surface) => Ok(Some((true, Some((owner, surface))))),
        ScopeObservation::MatchedOwnSurface => Ok(Some((true, None))),
        ScopeObservation::Unmatched { complete } => Ok(complete.then_some((false, None))),
        ScopeObservation::ProcessGone(state) => Err(process_gone(query_raw, state, deadline)),
        ScopeObservation::Unresponsive(state) => Err(super::owner_unavailable(state)),
    }
}

pub(crate) fn observed_unresponsive(observed: &Observed) -> bool {
    matches!(observed, Err(AppError::Adapter(error)) if error.code == ErrorCode::AppUnresponsive)
}

pub(crate) fn is_decided(error: &AppError) -> bool {
    matches!(
        error,
        AppError::Adapter(error) if error.details.as_ref().is_some_and(|details| {
            details["kind"] == TARGET_GONE || details["kind"] == OWNER_UNRESPONSIVE
        })
    )
}

/// The process the wait followed exited or crashed, so the selector can no
/// longer appear in any of its windows.
pub(crate) fn process_gone(query_raw: &str, state: ProcessState, deadline: Deadline) -> AppError {
    AdapterError::new(
        ErrorCode::AppNotFound,
        format!(
            "The application {} before the selector appeared",
            state.label()
        ),
    )
    .with_suggestion(
        "The action was delivered. Relaunch the application and read its state before acting again.",
    )
    .with_details(json!({
        "kind": TARGET_GONE,
        "predicate": "selector",
        "selector": query_raw,
        "process_state": state.label(),
        "retryable": false,
        "elapsed_ms": deadline.elapsed().as_millis(),
        "timeout_ms": deadline.timeout_ms(),
    }))
    .into()
}

/// The owner answered no read for [`crate::wait::unresponsive_spell::UNRESPONSIVE_LIMIT`].
pub(crate) fn owner_unresponsive(
    entry: &RefEntry,
    query_raw: &str,
    spell: std::time::Duration,
    deadline: Deadline,
) -> AppError {
    let app = entry
        .source
        .source_app
        .as_deref()
        .unwrap_or("target application");
    AdapterError::app_unresponsive(app)
        .with_details(json!({
            "kind": OWNER_UNRESPONSIVE,
            "predicate": "selector",
            "selector": query_raw,
            "process_state": ProcessState::Unresponsive.label(),
            "unresponsive_ms": spell.as_millis(),
            "retryable": false,
            "elapsed_ms": deadline.elapsed().as_millis(),
            "timeout_ms": deadline.timeout_ms(),
        }))
        .into()
}

#[cfg(test)]
#[path = "wait_selector_scope_tests.rs"]
mod tests;
