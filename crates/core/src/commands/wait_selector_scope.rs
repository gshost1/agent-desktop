//! What a post-action `--wait-for` observes for a ref it follows, and the
//! outcomes that end it before its budget.
//!
//! An action often shows its effect outside the window it was sent to: a
//! button opens a dialog, a menu item opens a new window, an OK button closes
//! its own dialog. The wait therefore reads the ref's window first, then the
//! windows its process opened since the action, and every window of the
//! process once two reads in a row found the ref's window closed. A ref on an
//! application surface, such as a menu bar item, reads that surface, then the
//! process's focused window and the windows opened since the action. A sheet,
//! alert or popover is not part of its window's tree, so a read no window
//! satisfies goes on to the overlays the process has open. A process
//! that exited or crashed can show nothing more, so the wait ends at once; one
//! whose reads keep failing ends only once the platform's own process check
//! says it is unresponsive.

use std::time::Duration;

use serde_json::json;

use crate::{
    AdapterError, AppError, Deadline, ErrorCode, LocatorQuery, PlatformAdapter, RefEntry,
    SnapshotOwner, SnapshotSurface,
    commands::selector_observation::SelectorObservation,
    snapshot::{ref_owner_observation::RefOwnerObservation, surface_target},
    system::process_state::ProcessState,
    wait::{
        ref_wait::RefWait,
        unresponsive_spell::UnresponsiveSpell,
        window_baseline::{self, WINDOW_BASELINE_BUDGET, process_windows},
    },
};

#[path = "wait_selector_reads.rs"]
mod reads;
use reads::{Unmatched, observe_each, observe_owner, read_overlays};

const TARGET_GONE: &str = "wait_target_gone";
const OWNER_UNRESPONSIVE: &str = "wait_owner_unresponsive";
const ABSENT_READS_BEFORE_WIDENING: u32 = 2;
const PROCESS_STATE_BUDGET: Duration = Duration::from_secs(1);

/// One read of the wait: `Some((matches, owner))` for a complete read, with
/// the owner and surface that matched, or `None` for an incomplete one.
pub(crate) type Observed =
    Result<Option<(bool, Option<(SnapshotOwner, SnapshotSurface)>)>, AppError>;

/// The scope an appear wait follows from a window-owned ref, and what it
/// remembers between reads.
pub(crate) struct FollowedScope<'a> {
    target: RefWait<'a>,
    absent_reads: u32,
    unresponsive: UnresponsiveSpell,
}

impl<'a> FollowedScope<'a> {
    pub(crate) fn new(target: RefWait<'a>) -> Self {
        Self {
            target,
            absent_reads: 0,
            unresponsive: UnresponsiveSpell::default(),
        }
    }

    /// Reads the scope once. A read that keeps failing as unresponsive for
    /// the spell limit ends the wait only when the process check confirms it;
    /// any other answer starts the spell over.
    pub(crate) fn observe(
        &mut self,
        adapter: &dyn PlatformAdapter,
        (query, query_raw): (&LocatorQuery, &str),
        deadline: Deadline,
    ) -> Observed {
        let observed = self.read(adapter, (query, query_raw), deadline);
        if let Some(spell) = self.unresponsive.record(read_unresponsive(&observed)) {
            if confirmed_unresponsive(adapter, self.target.entry, deadline) {
                return Err(owner_unresponsive(
                    self.target.entry,
                    query_raw,
                    spell,
                    deadline,
                ));
            }
            self.unresponsive = UnresponsiveSpell::default();
        }
        observed
    }

    fn read(
        &mut self,
        adapter: &dyn PlatformAdapter,
        (query, query_raw): (&LocatorQuery, &str),
        deadline: Deadline,
    ) -> Observed {
        let observed = if adapter.is_application_surface(self.target.entry.source.source_surface) {
            self.read_application_ref(adapter, (query, query_raw), deadline)?
        } else {
            self.read_windows(adapter, (query, query_raw), deadline)?
        };
        if matches!(observed, Some((true, _))) {
            return Ok(observed);
        }
        Ok(
            match read_overlays(adapter, self.target.entry, query, deadline)? {
                Some(found) => Some((true, Some(found))),
                None => observed,
            },
        )
    }

    /// Reads the ref's own application surface, then the windows its command
    /// acts in. A menu item's effect never shows in the menu bar it came from.
    fn read_application_ref(
        &self,
        adapter: &dyn PlatformAdapter,
        (query, query_raw): (&LocatorQuery, &str),
        deadline: Deadline,
    ) -> Observed {
        let mut unmatched = Unmatched::new(true);
        match SelectorObservation::observe_ref(adapter, self.target.entry, query, deadline) {
            Ok(SelectorObservation::Selector {
                matches: true,
                complete: true,
            }) => return Ok(Some((true, None))),
            Ok(SelectorObservation::Selector { complete, .. }) => unmatched.record_read(complete),
            Ok(SelectorObservation::OwnerUnavailable(
                state @ (ProcessState::Exited { .. } | ProcessState::Crashed { .. }),
            )) => return Err(process_gone(query_raw, state, deadline)),
            Ok(SelectorObservation::OwnerUnavailable(state)) => {
                return Err(super::owner_unavailable(state));
            }
            Err(error) => unmatched.record_error(error),
        }
        let budget = deadline.capped(WINDOW_BASELINE_BUDGET);
        let windows = unmatched.listed(match self.target.baseline {
            Some(baseline) => baseline.acted_on(adapter, budget),
            None => window_baseline::acted_on_without_baseline(adapter, self.target.entry, budget),
        });
        observe_each(adapter, windows, query, deadline, unmatched)
    }

    fn read_windows(
        &mut self,
        adapter: &dyn PlatformAdapter,
        (query, query_raw): (&LocatorQuery, &str),
        deadline: Deadline,
    ) -> Observed {
        match surface_target::resolve_ref_owner(adapter, self.target.entry, deadline)? {
            RefOwnerObservation::Present(owner) => {
                self.absent_reads = 0;
                let (matched, complete) = observe_owner(adapter, &owner, query, deadline)?;
                if matched {
                    return Ok(Some((true, Some((owner, SnapshotSurface::Window)))));
                }
                let mut unmatched = Unmatched::new(complete);
                let opened = match self.target.baseline {
                    Some(baseline) => unmatched.listed(
                        baseline.opened_since(adapter, deadline.capped(WINDOW_BASELINE_BUDGET)),
                    ),
                    None => Vec::new(),
                };
                observe_each(adapter, opened, query, deadline, unmatched)
            }
            RefOwnerObservation::Absent(None) => {
                self.absent_reads = self.absent_reads.saturating_add(1);
                if self.absent_reads < ABSENT_READS_BEFORE_WIDENING {
                    return Ok(Some((false, None)));
                }
                every_window(adapter, self.target.entry, query, deadline)
            }
            RefOwnerObservation::Absent(Some(state)) => {
                Err(process_gone(query_raw, state, deadline))
            }
            RefOwnerObservation::Unavailable(state) => Err(super::owner_unavailable(state)),
        }
    }
}

/// Every window of the ref's process instance, once its own window closed.
fn every_window(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    query: &LocatorQuery,
    deadline: Deadline,
) -> Observed {
    let (Some(app), Some(process)) = (entry.source.source_app.as_deref(), entry.process.identity())
    else {
        return Ok(None);
    };
    let mut unmatched = Unmatched::new(true);
    let windows = unmatched.listed(process_windows(
        adapter,
        app,
        &process,
        deadline.capped(WINDOW_BASELINE_BUDGET),
    ));
    observe_each(adapter, windows, query, deadline, unmatched)
}

fn read_unresponsive(observed: &Observed) -> bool {
    matches!(observed, Err(AppError::Adapter(error)) if error.code == ErrorCode::AppUnresponsive)
}

fn confirmed_unresponsive(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    deadline: Deadline,
) -> bool {
    entry.process.identity().is_some_and(|process| {
        matches!(
            adapter.process_state(process, deadline.capped(PROCESS_STATE_BUDGET)),
            Ok(ProcessState::Unresponsive)
        )
    })
}

/// Whether an error is one of the outcomes that end a followed wait before
/// its budget.
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
fn process_gone(query_raw: &str, state: ProcessState, deadline: Deadline) -> AppError {
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

/// Every read failed as unresponsive for the spell limit and the process
/// check then reported the process unresponsive.
fn owner_unresponsive(
    entry: &RefEntry,
    query_raw: &str,
    spell: Duration,
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

#[cfg(test)]
#[path = "wait_selector_owner_tests.rs"]
mod owner_tests;
