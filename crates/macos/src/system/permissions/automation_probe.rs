//! A deadline around the Automation permission probe.
//!
//! `AEDeterminePermissionToAutomateTarget` asks the TCC daemon even when it may
//! not prompt. While a consent prompt from the same responsible app waits on
//! screen, that call blocks until someone answers it; 175 s was observed with a
//! "wants access to control System Events" prompt pending. Every command that
//! collects a permission report would hang with it, past its own timeout.

use agent_desktop_core::Deadline;
use std::sync::mpsc;
use std::time::Duration;

/// The probe status reported when the probe did not answer in time. It is no
/// OSStatus macOS returns, so it maps to an unknown permission state.
pub(crate) const PROBE_UNANSWERED: i32 = i32::MIN;

const AUTOMATION_PROBE_BUDGET: Duration = Duration::from_millis(1_500);

/// Runs `probe` on its own thread and waits for it within the deadline. A probe
/// that does not answer is left to finish on its thread; its late answer is
/// discarded.
pub(crate) fn probe_within(deadline: Deadline, probe: fn() -> i32) -> i32 {
    let wait = deadline.remaining().min(AUTOMATION_PROBE_BUDGET);
    if wait.is_zero() {
        return PROBE_UNANSWERED;
    }
    let (sender, receiver) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("automation-permission-probe".into())
        .spawn(move || {
            let _ = sender.send(probe());
        });
    if spawned.is_err() {
        return PROBE_UNANSWERED;
    }
    receiver.recv_timeout(wait).unwrap_or(PROBE_UNANSWERED)
}

#[cfg(test)]
#[path = "automation_probe_tests.rs"]
mod tests;
