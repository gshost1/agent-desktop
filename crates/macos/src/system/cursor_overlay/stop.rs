use agent_desktop_core::{AdapterError, CursorOverlayControl, ErrorCode};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use super::exchange::{Exchange, exchange, is_closed, is_timed_out};

const RECHECK_INTERVAL: Duration = Duration::from_millis(10);

/// Stops the overlay listening on `socket`. An overlay that ends the stream
/// before acknowledging is exiting on its own, usually because it saw the
/// ended session first, so the endpoint is read again until it is gone or
/// acknowledges. Only an overlay still listening at the deadline fails.
pub(super) fn stop_until(
    socket: &Path,
    control: &CursorOverlayControl,
    deadline: Instant,
) -> Result<(), AdapterError> {
    loop {
        let error = match exchange(socket, control, deadline)? {
            Exchange::Absent | Exchange::Delivered => return Ok(()),
            Exchange::Unanswered(error) => error,
        };
        if !is_closed(&error) {
            return Err(not_stopped(error));
        }
        if Instant::now() >= deadline {
            return Err(still_listening(error));
        }
        thread::sleep(RECHECK_INTERVAL);
    }
}

fn not_stopped(error: std::io::Error) -> AdapterError {
    if is_timed_out(&error) {
        return still_listening(error);
    }
    AdapterError::internal("macOS cursor overlay did not acknowledge the stop")
        .with_platform_detail(error.to_string())
}

fn still_listening(error: std::io::Error) -> AdapterError {
    AdapterError::new(
        ErrorCode::Timeout,
        "macOS cursor overlay was still running at the stop deadline",
    )
    .with_suggestion(
        "The overlay of an ended session stops by itself within about a second; run 'session list' to confirm the session ended",
    )
    .with_platform_detail(error.to_string())
}

#[cfg(test)]
#[path = "stop_tests.rs"]
mod tests;
