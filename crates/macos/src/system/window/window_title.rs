//! CoreGraphics omits `kCGWindowName` when the caller lacks Screen Recording,
//! while the accessibility window of the same number still publishes `AXTitle`.
//! Records join the accessibility title before focus matching and before they
//! become `WindowInfo`, so the application name stands in only when neither
//! source has a title.

use crate::system::window::cg_window::WindowRecord;
use agent_desktop_core::AdapterError;
use rustc_hash::{FxHashMap, FxHashSet};

const OWNER_PROBE_BUDGET: std::time::Duration = std::time::Duration::from_millis(50);

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct WindowTitleProbe {
    pub(crate) accessible_window_ids: FxHashSet<i64>,
    pub(crate) title_by_id: FxHashMap<i64, String>,
}

/// Never replaces a CoreGraphics title that exists.
pub(crate) fn fill_missing_titles<'a>(
    records: &mut [WindowRecord],
    mut ax_title: impl FnMut(i32, i64) -> Option<&'a str>,
) {
    for record in records
        .iter_mut()
        .filter(|record| record.title.as_deref().is_none_or(str::is_empty))
    {
        record.title = ax_title(record.pid, record.window_number)
            .filter(|title| !title.is_empty())
            .map(str::to_owned);
    }
}

pub(crate) fn untitled_window_ids(records: &[WindowRecord], pid: i32) -> FxHashSet<i64> {
    records
        .iter()
        .filter(|record| record.pid == pid && record.title.as_deref().is_none_or(str::is_empty))
        .map(|record| record.window_number)
        .collect()
}

/// A failed probe leaves its owner out of the map, so its windows keep their
/// CoreGraphics titles and their accessibility label stays unknown.
pub(crate) fn probe_owners_with(
    records: &[WindowRecord],
    mut probe: impl FnMut(i32, &FxHashSet<i64>) -> Result<WindowTitleProbe, AdapterError>,
) -> FxHashMap<i32, WindowTitleProbe> {
    let mut pids = records.iter().map(|record| record.pid).collect::<Vec<_>>();
    pids.sort_unstable();
    pids.dedup();
    pids.into_iter()
        .filter_map(|pid| {
            probe(pid, &untitled_window_ids(records, pid))
                .ok()
                .map(|result| (pid, result))
        })
        .collect()
}

/// Bounds one owner's probe so a slow application cannot hold the global
/// inventory for longer than a short slice of the caller's deadline.
pub(crate) fn owner_probe_until(
    pid: i32,
    untitled: &FxHashSet<i64>,
    deadline: std::time::Instant,
) -> Result<WindowTitleProbe, AdapterError> {
    let probe_deadline = std::time::Instant::now()
        .checked_add(OWNER_PROBE_BUDGET)
        .map_or(deadline, |probe| probe.min(deadline));
    crate::system::window::window_ax_state::title_probe_until(pid, untitled, probe_deadline)
}

/// The global inventory probes every owner when it lists all windows, so
/// untitled records take their accessibility titles before focus matching. A
/// focused-only listing skips that walk and takes the focused window's title
/// from the frontmost accessibility read instead.
pub(crate) fn titled_records_until(
    eligible_pids: &FxHashSet<i32>,
    focused_only: bool,
    deadline: std::time::Instant,
    probes: &mut FxHashMap<i32, WindowTitleProbe>,
) -> Result<Vec<WindowRecord>, AdapterError> {
    let mut records = crate::system::window::cg_window::window_records_until(
        deadline,
        crate::system::window::cg_window::WindowRecordScope::Pids(eligible_pids),
    )?;
    if !focused_only {
        *probes = probe_owners_with(&records, |pid, untitled| {
            owner_probe_until(pid, untitled, deadline)
        });
        fill_from_probes(&mut records, probes);
    }
    Ok(records)
}

pub(crate) fn accessible_ids(
    probes: &FxHashMap<i32, WindowTitleProbe>,
    pid: i32,
    focused_only: bool,
    deadline: std::time::Instant,
) -> Option<FxHashSet<i64>> {
    match probes.get(&pid) {
        Some(probe) => Some(probe.accessible_window_ids.clone()),
        None if focused_only => owner_probe_until(pid, &FxHashSet::default(), deadline)
            .ok()
            .map(|probe| probe.accessible_window_ids),
        None => None,
    }
}

pub(crate) fn fill_from_probes(
    records: &mut [WindowRecord],
    probes: &FxHashMap<i32, WindowTitleProbe>,
) {
    fill_missing_titles(records, |pid, window_number| {
        probes
            .get(&pid)
            .and_then(|probe| probe.title_by_id.get(&window_number))
            .map(String::as_str)
    });
}

#[cfg(test)]
#[path = "window_title_tests.rs"]
mod tests;
