use agent_desktop_core::{AdapterError, CursorOverlayControl};
use std::path::Path;
use std::time::Instant;

pub(super) fn stop_until(
    socket: &Path,
    control: &CursorOverlayControl,
    deadline: Instant,
) -> Result<(), AdapterError> {
    super::spawn::send_until(socket, control, deadline).map(|_| ())
}

#[cfg(test)]
#[path = "stop_tests.rs"]
mod tests;
