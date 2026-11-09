//! A deadline around the Automation permission probe.
//!
//! `AEDeterminePermissionToAutomateTarget` asks the TCC daemon even when it may
//! not prompt. While a consent prompt from the same responsible app waits on
//! screen, that call blocks until someone answers it; 175 s was observed with a
//! "wants access to control System Events" prompt pending. Every command that
//! collects a full permission report would hang with it, past its own timeout.

use agent_desktop_core::{AdapterError, Deadline};
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

/// The probe status reported when the probe did not answer in time. It is no
/// OSStatus macOS returns, so it maps to an unknown permission state.
pub(crate) const PROBE_UNANSWERED: i32 = i32::MIN;

const AUTOMATION_PROBE_BUDGET: Duration = Duration::from_secs(3);

/// A probe that did not answer keeps running on its thread. The next caller
/// waits on that same probe instead of starting another, so a host that keeps
/// asking while a prompt waits holds one blocked thread, not one per call.
static IN_FLIGHT: Mutex<Option<Receiver<i32>>> = Mutex::new(None);

pub(crate) fn probe_within(deadline: Deadline, probe: fn() -> i32) -> Result<i32, AdapterError> {
    probe_with(
        &IN_FLIGHT,
        deadline.remaining_slice(AUTOMATION_PROBE_BUDGET)?,
        probe,
    )
}

pub(crate) fn probe_with(
    slot: &Mutex<Option<Receiver<i32>>>,
    wait: Duration,
    probe: fn() -> i32,
) -> Result<i32, AdapterError> {
    let pending = slot.lock().ok().and_then(|mut held| held.take());
    let receiver = match pending {
        Some(receiver) => receiver,
        None => match spawn(probe) {
            Some(receiver) => receiver,
            None => return Ok(PROBE_UNANSWERED),
        },
    };
    match receiver.recv_timeout(wait) {
        Ok(status) => Ok(status),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            if let Ok(mut held) = slot.lock()
                && held.is_none()
            {
                *held = Some(receiver);
            }
            Ok(PROBE_UNANSWERED)
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Ok(PROBE_UNANSWERED),
    }
}

fn spawn(probe: fn() -> i32) -> Option<Receiver<i32>> {
    let (sender, receiver) = mpsc::channel();
    std::thread::Builder::new()
        .name("automation-permission-probe".into())
        .spawn(move || {
            let _ = sender.send(probe());
        })
        .ok()?;
    Some(receiver)
}

#[cfg(test)]
#[path = "automation_probe_tests.rs"]
mod tests;
