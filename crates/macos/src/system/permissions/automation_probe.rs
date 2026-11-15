//! A deadline around the Automation permission probe.
//!
//! `AEDeterminePermissionToAutomateTarget` asks the TCC daemon even when it may
//! not prompt. While a consent prompt from the same responsible app waits on
//! screen, that call blocks until someone answers it; 175 s was observed with a
//! "wants access to control System Events" prompt pending. Every command that
//! collects a full permission report would hang with it, past its own timeout.

use agent_desktop_core::{AdapterError, Deadline};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

/// The probe status reported when the probe did not answer in time. It is no
/// OSStatus macOS returns, so it maps to an unknown permission state.
pub(crate) const PROBE_UNANSWERED: i32 = i32::MIN;

const AUTOMATION_PROBE_BUDGET: Duration = Duration::from_secs(3);

/// One probe's answer, set once by its thread and read by every caller that
/// waits on it.
pub(crate) type SharedProbe = Arc<(Mutex<Option<i32>>, Condvar)>;

/// The probe in flight. Every caller, concurrent or later, waits on this one
/// probe while it has not answered, so a host that keeps asking while a
/// prompt waits holds one blocked thread, not one per call. The first caller
/// to read the answer clears the slot, so the next call asks again.
static IN_FLIGHT: Mutex<Option<SharedProbe>> = Mutex::new(None);

pub(crate) fn probe_within(deadline: Deadline, probe: fn() -> i32) -> Result<i32, AdapterError> {
    probe_with(
        &IN_FLIGHT,
        deadline.remaining_slice(AUTOMATION_PROBE_BUDGET)?,
        probe,
    )
}

pub(crate) fn probe_with(
    slot: &Mutex<Option<SharedProbe>>,
    wait: Duration,
    probe: fn() -> i32,
) -> Result<i32, AdapterError> {
    let Some(shared) = join_or_start(slot, probe) else {
        return Ok(PROBE_UNANSWERED);
    };
    let Some(status) = wait_for(&shared, wait) else {
        return Ok(PROBE_UNANSWERED);
    };
    if let Ok(mut held) = slot.lock()
        && held
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, &shared))
    {
        *held = None;
    }
    Ok(status)
}

fn join_or_start(slot: &Mutex<Option<SharedProbe>>, probe: fn() -> i32) -> Option<SharedProbe> {
    let mut held = slot.lock().ok()?;
    if let Some(shared) = held.as_ref() {
        return Some(Arc::clone(shared));
    }
    let shared = spawn(probe)?;
    *held = Some(Arc::clone(&shared));
    Some(shared)
}

fn wait_for(shared: &SharedProbe, wait: Duration) -> Option<i32> {
    let (status, answered) = &**shared;
    let guard = status.lock().ok()?;
    let (guard, _) = answered
        .wait_timeout_while(guard, wait, |status| status.is_none())
        .ok()?;
    *guard
}

fn spawn(probe: fn() -> i32) -> Option<SharedProbe> {
    let shared: SharedProbe = Arc::new((Mutex::new(None), Condvar::new()));
    let answer = Arc::clone(&shared);
    std::thread::Builder::new()
        .name("automation-permission-probe".into())
        .spawn(move || {
            let status = std::panic::catch_unwind(probe).unwrap_or(PROBE_UNANSWERED);
            let (slot, answered) = &*answer;
            if let Ok(mut held) = slot.lock() {
                *held = Some(status);
            }
            answered.notify_all();
        })
        .ok()?;
    Some(shared)
}

#[cfg(test)]
#[path = "automation_probe_tests.rs"]
mod tests;
