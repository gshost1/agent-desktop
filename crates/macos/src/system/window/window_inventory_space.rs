//! Marks the windows of one application's inventory that sit on another
//! Space, so `list-windows --app` lists them instead of dropping them.
//!
//! macOS removes a window on another Space, or on the desktop behind a
//! full-screen Space, from its application's `AXWindows`, while CoreGraphics
//! still lists it off screen. Without the accessibility read such a window has
//! no minimized state, and the inventory would treat it like a window the
//! application ordered out. The window server tells the two apart: an
//! ordered-out window belongs to no Space.
//!
//! Only off-screen windows with a real size that are neither minimized nor
//! published through accessibility are asked about, at most
//! [`MAX_SPACE_READS`] of them, within [`SPACE_READ_BUDGET`], in one worker.
//! An inventory whose windows are all on screen or published makes no read.
//!
//! The read is best effort, so the last placement read for each window number
//! is remembered for the life of the process. A window the current read could
//! not answer for keeps its remembered placement, so a window on another Space
//! does not drop out of one poll of a wait and come back in the next. A fresh
//! answer always replaces the remembered one. A window never answered for is
//! not marked and stays out.

use agent_desktop_core::WindowInfo;
use rustc_hash::FxHashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const MAX_SPACE_READS: usize = 32;
const SPACE_READ_BUDGET: Duration = Duration::from_millis(100);
const MAX_REMEMBERED: usize = 4096;

static REMEMBERED: Mutex<Option<FxHashMap<i64, bool>>> = Mutex::new(None);

/// Sets `on_other_space` on every candidate window that `read_placement`
/// places on another Space, or that an earlier read placed there when this
/// read has no answer for it. `read_placement` is called once, and only when
/// there is a candidate.
pub(crate) fn mark_other_space_windows(
    windows: &mut [WindowInfo],
    read_placement: impl FnOnce(Vec<i64>) -> FxHashMap<i64, bool>,
) {
    let lock = || {
        REMEMBERED
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    };
    let mut remembered = lock().clone().unwrap_or_default();
    let before = remembered.clone();
    mark_with_memory(windows, read_placement, &mut remembered);
    if remembered != before {
        let mut shared = lock();
        let shared = shared.get_or_insert_with(FxHashMap::default);
        if remembered.len() < before.len() {
            shared.clear();
        }
        shared.extend(remembered);
    }
}

fn mark_with_memory(
    windows: &mut [WindowInfo],
    read_placement: impl FnOnce(Vec<i64>) -> FxHashMap<i64, bool>,
    remembered: &mut FxHashMap<i64, bool>,
) {
    let candidates = windows
        .iter()
        .filter(|window| is_space_candidate(window))
        .filter_map(window_number)
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return;
    }
    let mut placements = read_placement(candidates.iter().copied().take(MAX_SPACE_READS).collect());
    if remembered.len() + placements.len() > MAX_REMEMBERED {
        remembered.clear();
    }
    for number in candidates {
        match placements.get(&number) {
            Some(&placement) => {
                remembered.insert(number, placement);
            }
            None => {
                if let Some(&placement) = remembered.get(&number) {
                    placements.insert(number, placement);
                }
            }
        }
    }
    for window in windows.iter_mut() {
        let on_other_space = is_space_candidate(window)
            && window_number(window)
                .is_some_and(|number| placements.get(&number).copied() == Some(true));
        if on_other_space {
            window.state.on_other_space = Some(true);
        }
    }
}

/// The deadline for the Space reads of one inventory.
pub(crate) fn space_read_deadline(deadline: Instant) -> Instant {
    Instant::now()
        .checked_add(SPACE_READ_BUDGET)
        .map_or(deadline, |budget| budget.min(deadline))
}

fn is_space_candidate(window: &WindowInfo) -> bool {
    window.state.visible == Some(false)
        && window.state.minimized.is_none()
        && !window.state.accessible
        && window
            .bounds
            .is_some_and(|bounds| bounds.width > 0.0 && bounds.height > 0.0)
}

fn window_number(window: &WindowInfo) -> Option<i64> {
    crate::system::window::window_resolve::parse_window_number(&window.id)
}

#[cfg(test)]
#[path = "window_inventory_space_tests.rs"]
mod tests;
