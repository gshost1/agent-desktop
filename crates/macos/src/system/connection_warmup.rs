//! Opens this process's connections to the window server and to a target
//! application's accessibility server on background threads.
//!
//! Every command runs as a fresh process. The first window-server query and the
//! first accessibility message to an application each spend 25-40 ms setting
//! up a connection before any data moves, and on one thread those setups run
//! one after another. Starting them early on their own threads lets them
//! overlap each other and the work the command does before it needs them; the
//! command's own reads then find the connection already open. The warm-up
//! reads are passive, their results are discarded, and no command result
//! depends on them.

use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// A command aims at one application, which may run as a few processes. A
/// larger set is an unscoped inventory, and warming it would message
/// applications the command never reads.
const MAX_WARMED_APPLICATIONS: usize = 4;
/// The accessibility layer caps one message at 250 ms; a warm-up never needs
/// more than one message.
const APPLICATION_READ_LIMIT: Duration = Duration::from_millis(250);
const EXIT_WAIT_LIMIT: Duration = Duration::from_millis(300);

static IN_FLIGHT: Mutex<usize> = Mutex::new(0);
static IDLE: Condvar = Condvar::new();
static WINDOW_SERVER: OnceLock<()> = OnceLock::new();
static EXIT_HOOK: OnceLock<()> = OnceLock::new();
static WARMED_APPLICATIONS: Mutex<Vec<i32>> = Mutex::new(Vec::new());

/// Starts the window-server connection once per process.
pub(crate) fn start_window_server() {
    WINDOW_SERVER.get_or_init(|| {
        spawn(|| {
            let _ = crate::system::window::cg_window::window_dictionaries();
        })
    });
}

/// Starts the accessibility connection to each process of a command's target
/// application once. An inventory of more processes than one application
/// runs, or a deadline that has passed, starts nothing.
pub(crate) fn start_applications(pids: &[i32], deadline: Instant) {
    let now = Instant::now();
    if pids.len() > MAX_WARMED_APPLICATIONS || now >= deadline {
        return;
    }
    let read_deadline = deadline.min(now + APPLICATION_READ_LIMIT);
    let Ok(mut warmed) = WARMED_APPLICATIONS.lock() else {
        return;
    };
    for &pid in pids {
        if admit(&mut warmed, pid) {
            spawn(move || read_application_role(pid, read_deadline));
        }
    }
}

/// Starts the accessibility connection to one process a command named
/// before its own deadline exists, bounded by the one-message limit.
pub(crate) fn start_application(pid: u32) {
    if let Ok(pid) = i32::try_from(pid) {
        start_applications(&[pid], Instant::now() + APPLICATION_READ_LIMIT);
    }
}

fn admit(warmed: &mut Vec<i32>, pid: i32) -> bool {
    if pid <= 0 || warmed.contains(&pid) || warmed.len() >= MAX_WARMED_APPLICATIONS {
        return false;
    }
    warmed.push(pid);
    true
}

fn read_application_role(pid: i32, deadline: Instant) {
    use core_foundation::{base::TCFType, string::CFString};
    let application = crate::tree::element_for_pid(pid);
    let attribute = CFString::from_static_string("AXRole");
    let (_, value) = crate::tree::properties::ax_ipc::copy_attribute_value(
        &application,
        attribute.as_concrete_TypeRef(),
        deadline,
    );
    if !value.is_null() {
        unsafe { core_foundation::base::CFRelease(value) };
    }
}

fn spawn(work: impl FnOnce() + Send + 'static) {
    EXIT_HOOK.get_or_init(|| {
        unsafe { libc::atexit(wait_at_exit) };
    });
    let Ok(mut count) = IN_FLIGHT.lock() else {
        return;
    };
    *count += 1;
    drop(count);
    let spawned = std::thread::Builder::new()
        .name("agent-desktop-connection-warmup".into())
        .spawn(move || {
            let _finished = Finished;
            work();
        });
    if spawned.is_err() {
        finish();
    }
}

/// Counts a warm-up as finished however its thread ends, so a panic inside a
/// framework call cannot leave exit waiting on it.
struct Finished;

impl Drop for Finished {
    fn drop(&mut self) {
        finish();
    }
}

fn finish() {
    let Ok(mut count) = IN_FLIGHT.lock() else {
        return;
    };
    *count = count.saturating_sub(1);
    if *count == 0 {
        IDLE.notify_all();
    }
}

/// Exiting while a warm-up thread is inside a framework call could race the
/// framework's own teardown, so exit waits for warm-ups still running. The
/// limit covers the longest a warm-up message can take, and a command that
/// finishes after its warm-ups waits for nothing.
extern "C" fn wait_at_exit() {
    let Ok(count) = IN_FLIGHT.lock() else {
        return;
    };
    let _ = IDLE.wait_timeout_while(count, EXIT_WAIT_LIMIT, |count| *count > 0);
}

#[cfg(test)]
#[path = "connection_warmup_tests.rs"]
mod tests;
