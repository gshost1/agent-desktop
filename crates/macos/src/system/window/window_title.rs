//! CoreGraphics omits `kCGWindowName` when the caller lacks Screen Recording,
//! while the accessibility window of the same number still publishes `AXTitle`.
//! Scoped inventory, global inventory and strict resolution all title their
//! records through `join_ax_title`, before focus matching and before a record
//! becomes `WindowInfo`, so the application name stands in only when neither
//! source has a title.
//!
//! Every path reads `AXTitle` in the same walk that reads accessible window ids
//! (`window_ax_state::read_windows_until`). The global inventory bounds each
//! owner's walk to `OWNER_PROBE_BUDGET` and the scoped paths spend the
//! caller's deadline. In both, a walk that does not finish yields no titles,
//! never some of them, and where the path tolerates that failure the owner's
//! untitled windows keep the application name. The budgets differ, so a slow
//! owner can be titled by a scoped listing and not by the global one.

use crate::system::window::{cg_window::WindowRecord, window_ax_state::WindowAxState};
use agent_desktop_core::AdapterError;
use rustc_hash::{FxHashMap, FxHashSet};
use std::time::{Duration, Instant};

const OWNER_PROBE_BUDGET: Duration = Duration::from_millis(50);

/// Never replaces a CoreGraphics title that exists. Both sources normalize an
/// empty title to `None` where they are decoded.
pub(crate) fn join_ax_title(record: &mut WindowRecord, state: Option<&WindowAxState>) {
    if record.title.is_some() {
        return;
    }
    record.title = state
        .and_then(|state| state.title_by_id.get(&record.window_number))
        .cloned();
}

/// Walks each owner once under a short budget. A failed walk leaves its owner
/// out of the map, so its windows keep their CoreGraphics titles and their
/// accessibility label stays unknown.
pub(crate) fn probe_owners_until(
    records: &[WindowRecord],
    pids: &[i32],
    deadline: Instant,
) -> FxHashMap<i32, WindowAxState> {
    probe_owners_with(records, pids, |pid, untitled| {
        let probe_deadline = Instant::now()
            .checked_add(OWNER_PROBE_BUDGET)
            .map_or(deadline, |probe| probe.min(deadline));
        crate::system::window::window_ax_state::read_windows_until(pid, probe_deadline, |id| {
            untitled.contains(&id)
        })
    })
}

/// Only windows without a CoreGraphics title need their accessibility fields;
/// the walk still lists every accessible id.
pub(crate) fn probe_owners_with(
    records: &[WindowRecord],
    pids: &[i32],
    mut probe: impl FnMut(i32, &FxHashSet<i64>) -> Result<WindowAxState, AdapterError>,
) -> FxHashMap<i32, WindowAxState> {
    pids.iter()
        .filter_map(|pid| {
            let untitled = records
                .iter()
                .filter(|record| record.pid == *pid && record.title.is_none())
                .map(|record| record.window_number)
                .collect();
            probe(*pid, &untitled).ok().map(|state| (*pid, state))
        })
        .collect()
}

#[cfg(test)]
#[path = "window_title_tests.rs"]
mod tests;
