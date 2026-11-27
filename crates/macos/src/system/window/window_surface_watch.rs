//! Watching a sheet or dialog through its own window-server window. Accessibility
//! can list a sheet as gone while it is still on screen, and a window's own
//! list churns while its content fills, so a surface counts as closed only
//! when the window server shows its window off screen on two consecutive reads.

use agent_desktop_core::{AdapterError, Deadline};
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(20);
pub(crate) const SURFACE_WINDOW_CLOSED: &str = "surface_window_closed";

/// Polls `read` until the same signal is seen on two consecutive reads, or
/// `limit` passes. A signal that one re-read does not confirm is not an
/// effect.
pub(crate) fn settle_with(
    mut read: impl FnMut() -> Result<Option<&'static str>, AdapterError>,
    limit: Duration,
    deadline: Deadline,
) -> Result<Option<&'static str>, AdapterError> {
    let soft = Instant::now() + deadline.capped(limit).remaining();
    let hard = soft + POLL_INTERVAL * 3;
    let mut previous = None;
    loop {
        let signal = read()?;
        if signal.is_some() && signal == previous {
            return Ok(signal);
        }
        previous = signal;
        let now = Instant::now();
        if now >= hard || (now >= soft && previous.is_none()) {
            return Ok(None);
        }
        let pause = deadline.remaining_slice(POLL_INTERVAL)?;
        std::thread::sleep(pause.min(POLL_INTERVAL));
    }
}

/// The surface's own window, only when it differs from the window it is
/// attached to; a bridge that answers with the host's window would make the
/// surface look open forever or closed with the host.
pub(crate) fn distinct_window(own: Option<i64>, host: Option<i64>) -> Option<i64> {
    own.filter(|own| host != Some(*own))
}

pub(crate) fn window_on_screen(window: i64) -> Result<bool, AdapterError> {
    use super::cg_window::{WindowRecordScope, records_from_dictionaries, window_dictionaries};

    Ok(
        records_from_dictionaries(window_dictionaries()?, WindowRecordScope::Window(window))?
            .iter()
            .any(|record| record.presence.is_onscreen()),
    )
}

/// The closed signal for `window`, or nothing while it is still on screen.
pub(crate) fn closed_signal(window: i64) -> Result<Option<&'static str>, AdapterError> {
    Ok((!window_on_screen(window)?).then_some(SURFACE_WINDOW_CLOSED))
}

#[cfg(test)]
#[path = "window_surface_watch_tests.rs"]
mod tests;
