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

const MAX_WARMED_APPLICATIONS: usize = 4;
const APPLICATION_READ_LIMIT: Duration = Duration::from_secs(1);
const EXIT_WAIT_LIMIT: Duration = Duration::from_millis(100);

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

/// Starts the accessibility connection to each application process once, for
/// at most a few processes.
pub(crate) fn start_applications(pids: impl IntoIterator<Item = i32>) {
    let Ok(mut warmed) = WARMED_APPLICATIONS.lock() else {
        return;
    };
    for pid in pids {
        if pid <= 0 || warmed.contains(&pid) || warmed.len() >= MAX_WARMED_APPLICATIONS {
            continue;
        }
        warmed.push(pid);
        spawn(move || read_application_role(pid));
    }
}

fn read_application_role(pid: i32) {
    use core_foundation::{base::TCFType, string::CFString};
    let application = crate::tree::element_for_pid(pid);
    let attribute = CFString::from_static_string("AXRole");
    let (_, value) = crate::tree::properties::ax_ipc::copy_attribute_value(
        &application,
        attribute.as_concrete_TypeRef(),
        Instant::now() + APPLICATION_READ_LIMIT,
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
            work();
            finish();
        });
    if spawned.is_err() {
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
/// framework's own teardown, so exit waits briefly for warm-ups still running.
/// A warm-up blocked on an unresponsive application is left behind after the
/// limit; it is waiting in a kernel message receive by then.
extern "C" fn wait_at_exit() {
    let Ok(count) = IN_FLIGHT.lock() else {
        return;
    };
    let _ = IDLE.wait_timeout_while(count, EXIT_WAIT_LIMIT, |count| *count > 0);
}

#[cfg(test)]
pub(crate) fn in_flight() -> usize {
    IN_FLIGHT.lock().map(|count| *count).unwrap_or(usize::MAX)
}

#[cfg(test)]
pub(crate) fn window_server_started() -> bool {
    WINDOW_SERVER.get().is_some()
}

#[cfg(test)]
pub(crate) fn warmed_applications() -> Vec<i32> {
    WARMED_APPLICATIONS
        .lock()
        .map(|warmed| warmed.clone())
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "connection_warmup_tests.rs"]
mod tests;
