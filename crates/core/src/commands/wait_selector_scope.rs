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
    /// Whether an appear wait follows this ref past its window: refs owned by
    /// an application surface or by a process root already observe the whole
    /// owner, and a gone wait keeps the scope the selector was in.
    pub(crate) fn follows(&self, adapter: &dyn PlatformAdapter, gone: bool) -> bool {
        !gone
            && !adapter.is_application_surface(self.entry.source.source_surface)
            && !self.entry.source.is_process_owned()
    }
}

/// One read of a followed scope.
pub(crate) enum ScopeObservation {
    Matched(SnapshotOwner),
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
            }) => return Ok(ScopeObservation::Matched(owner)),
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

pub(crate) type Observed = Result<Option<(bool, Option<crate::SnapshotOwner>)>, AppError>;

/// Reads a followed scope. A gone process is decided at once; an unresponsive
/// one is a retryable read the caller times by its spell.
pub(crate) fn observe_followed(
    adapter: &dyn PlatformAdapter,
    target: RefWait<'_>,
    (query, query_raw): (&crate::LocatorQuery, &str),
    deadline: crate::Deadline,
) -> Observed {
    match observe(adapter, target, query, deadline)? {
        ScopeObservation::Matched(owner) => Ok(Some((true, Some(owner)))),
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
