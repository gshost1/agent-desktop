//! Explains a launch that reports no window while the application holds one
//! off screen.
//!
//! While another app is full screen, or the window sits on another Space,
//! belongs to a hidden app or is kept hidden by the app, the window server
//! still lists the window, off screen and with its real size, but macOS gives
//! it no accessibility tree. The public window list cannot tell these cases
//! apart, so the note names them all. The launch cannot report the window,
//! and every later command on the app fails `WINDOW_NOT_FOUND`. A fresh
//! launch with `--activate` opens the window on the current Space; activating
//! an app that is already running does not move its window there.

use std::time::Instant;

use agent_desktop_core::{Deadline, system::launch_result::LaunchResult};

use crate::system::window::cg_window::{WindowRecord, WindowRecordScope, window_records_until};

pub(crate) const OFF_SPACE_NOTE: &str = "No window is reported, but the window server lists an off-screen window of this process that macOS gives no accessibility tree: it may be on another Space (for example behind a full-screen app), belong to a hidden app, or be kept hidden by the app until it is brought forward. Commands on the app fail WINDOW_NOT_FOUND until the window is on the current Space. A launch with --activate of an app that is not running yet opens its window on the current Space; for a running app, leave full screen in the front app or switch to the window's Space, then retry.";

/// Adds the off-Space note to a result that reports no window and carries no
/// other note, when the launched process has an off-screen window.
pub(crate) fn explain_off_space(result: &mut LaunchResult, pid: i32, deadline: Deadline) {
    if result.window.is_some() || result.notes.suggestion.is_some() {
        return;
    }
    let until = Instant::now() + deadline.remaining();
    let Ok(records) = window_records_until(until, WindowRecordScope::Pid(pid)) else {
        return;
    };
    if has_off_space_window(&records) {
        result.notes.suggestion = Some(OFF_SPACE_NOTE.to_owned());
    }
}

/// A window the window server lists off screen with a real size. A window
/// that is still being created has no size yet.
pub(crate) fn has_off_space_window(records: &[WindowRecord]) -> bool {
    records
        .iter()
        .any(|record| !record.visible && record.bounds.width > 0.0 && record.bounds.height > 0.0)
}

#[cfg(test)]
#[path = "launch_off_space_tests.rs"]
mod tests;
