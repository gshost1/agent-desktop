use std::time::{Duration, Instant};

use agent_desktop_core::{AdapterError, ErrorCode, WindowInfo};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::system::window::window_ax_titles::WindowAxTitles;

/// Fast accessibility labeling budget per application.
const LABEL_PROBE: Duration = Duration::from_millis(50);
/// Shared retry budget for the entire inventory.
const LABEL_RETRY_BUDGET: Duration = Duration::from_millis(500);

pub(super) fn label_until(
    windows: &mut [WindowInfo],
    deadline: Instant,
    mut read: impl FnMut(i32, &FxHashSet<i64>, Instant) -> Result<WindowAxTitles, AdapterError>,
    mut now: impl FnMut() -> Instant,
) {
    let mut timed_out = Vec::new();
    let mut results = FxHashMap::default();
    super::super::window_ax_state::label_accessibility(windows, |pid, wanted| {
        let result = read(pid, wanted, probe_deadline(now(), deadline, LABEL_PROBE));
        if result
            .as_ref()
            .is_err_and(|error| error.code == ErrorCode::Timeout)
        {
            timed_out.push((pid, wanted.clone()));
        }
        results.insert(pid, result);
        Err(AdapterError::timeout("Labels pending"))
    });
    let retry_deadline = probe_deadline(now(), deadline, LABEL_RETRY_BUDGET);
    for (pid, wanted) in timed_out {
        if retry_deadline.saturating_duration_since(now()) <= LABEL_PROBE * 2 {
            break;
        }
        results.insert(pid, read(pid, &wanted, retry_deadline));
    }
    super::super::window_ax_state::label_accessibility(windows, |pid, _| {
        results
            .remove(&pid)
            .unwrap_or_else(|| Err(AdapterError::timeout("No label read")))
    });
}

fn probe_deadline(now: Instant, deadline: Instant, budget: Duration) -> Instant {
    now.checked_add(budget)
        .map_or(deadline, |probe| probe.min(deadline))
}

#[cfg(test)]
#[path = "window_inventory_global_label_tests.rs"]
mod tests;
