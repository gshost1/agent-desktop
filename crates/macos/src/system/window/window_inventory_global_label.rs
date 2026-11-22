use std::time::{Duration, Instant};

use agent_desktop_core::{AdapterError, ErrorCode};
use rustc_hash::FxHashSet;

use crate::system::window::window_ax_titles::WindowAxTitles;

/// Fast accessibility labeling budget per application.
const LABEL_PROBE: Duration = Duration::from_millis(50);
/// One longer labeling attempt for a responsive application with a slow first read.
const LABEL_RETRY_PROBE: Duration = Duration::from_millis(500);

pub(super) fn read_until(
    pid: i32,
    wanted: &FxHashSet<i64>,
    deadline: Instant,
    mut read: impl FnMut(i32, &FxHashSet<i64>, Instant) -> Result<WindowAxTitles, AdapterError>,
) -> Result<WindowAxTitles, AdapterError> {
    match read(pid, wanted, probe_deadline(deadline, LABEL_PROBE)) {
        Err(error) if error.code == ErrorCode::Timeout => {
            read(pid, wanted, probe_deadline(deadline, LABEL_RETRY_PROBE))
        }
        result => result,
    }
}

fn probe_deadline(deadline: Instant, budget: Duration) -> Instant {
    Instant::now()
        .checked_add(budget)
        .map_or(deadline, |probe| probe.min(deadline))
}

#[cfg(test)]
#[path = "window_inventory_global_label_tests.rs"]
mod tests;
