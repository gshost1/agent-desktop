use std::time::{Duration, Instant};

use agent_desktop_core::{
    AdapterError, Deadline, ErrorCode, system::launch_options::LaunchOptions,
};

use super::launch_window::LaunchWindow;

pub(crate) const STARTUP_GRACE: Duration = Duration::from_millis(1500);
const FIRST_POLL: Duration = Duration::from_millis(25);
const POLL_LIMIT: Duration = Duration::from_millis(250);

/// The probes one launch wait reads on every poll.
pub(crate) struct SettleProbes<R, S, A> {
    /// One read of the launched process's windows.
    pub(crate) read: R,
    /// Whether the application reports that it finished starting up.
    pub(crate) startup_finished: S,
    /// `Err` once the launched process instance is gone.
    pub(crate) alive: A,
}

/// Waits only for the windows the launch itself produces. Starting up is what
/// creates them, so once the application reports that it finished starting up,
/// every window it was going to open on its own already exists and further
/// polling can only run out the deadline. An application that opens its first
/// window in response to being brought forward reports no window here, and
/// `activate` is how a caller asks for one instead of waiting for one it never
/// requested. The process is checked on every poll in every state, so a launch
/// whose application exits, before or after it finished starting up, ends with
/// that error instead of a pid for a process that is gone.
pub(crate) fn settle_window<R, S, A>(
    probes: SettleProbes<R, S, A>,
    options: &LaunchOptions,
    deadline: Deadline,
) -> Result<LaunchWindow, AdapterError>
where
    R: FnMut() -> Result<LaunchWindow, AdapterError>,
    S: FnMut() -> Result<bool, AdapterError>,
    A: FnMut() -> Result<(), AdapterError>,
{
    let SettleProbes {
        mut read,
        mut startup_finished,
        mut alive,
    } = probes;
    let mut poll_interval = FIRST_POLL;
    let mut grace_ends_at = None;
    loop {
        let last_read_failure = match read() {
            Ok(LaunchWindow::Absent) => None,
            Ok(found) => return Ok(found),
            Err(error) if window_read_is_not_ready_yet(&error) => Some(error),
            Err(error) => return Err(error),
        };
        alive()?;
        if options.timeout_ms == 0
            || (!options.presentation.activate && grace_over(grace_ends_at, Instant::now()))
        {
            return last_read_failure.map_or(Ok(LaunchWindow::Absent), Err);
        }
        if grace_ends_at.is_none() && startup_finished()? {
            grace_ends_at = Instant::now().checked_add(STARTUP_GRACE);
        }
        let remaining = deadline.remaining();
        if remaining.is_zero() {
            return last_read_failure.map_or(Ok(LaunchWindow::Absent), Err);
        }
        std::thread::sleep(poll_interval.min(remaining));
        poll_interval = (poll_interval * 3 / 2).min(POLL_LIMIT);
    }
}

pub(crate) fn window_read_is_not_ready_yet(error: &AdapterError) -> bool {
    matches!(error.code, ErrorCode::AppUnresponsive | ErrorCode::Timeout)
}

/// The grace covers the gap between an application reporting that it started
/// and its first window reaching the window server. Waiting starts running out
/// only once there is a completed startup to measure from.
pub(crate) fn grace_over(grace_ends_at: Option<Instant>, now: Instant) -> bool {
    grace_ends_at.is_some_and(|end| now >= end)
}

pub(crate) fn launch_target_gone(pid: i32) -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Launched application exited before it presented a window",
    )
    .with_details(serde_json::json!({ "pid": pid, "complete": false }))
    .with_suggestion("Check the application's own launch requirements, then retry.")
}

#[cfg(test)]
#[path = "launch_settle_tests.rs"]
mod tests;
