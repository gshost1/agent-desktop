//! A daemon stopped by a signal removes its socket and state file before it dies, so `kill` (or
//! a client that hung up mid-command) leaves no endpoint behind. The handler only unlinks the
//! two paths prepared before it was installed, then dies of the same signal with its default
//! action, which keeps the exit status a one-shot CLI would have.

use std::ffi::CString;
use std::os::unix::ffi::OsStringExt;
use std::sync::OnceLock;

use super::endpoint::DaemonEndpoint;

const SIGNALS: [libc::c_int; 3] = [libc::SIGTERM, libc::SIGINT, libc::SIGHUP];

static PATHS: OnceLock<[CString; 2]> = OnceLock::new();

/// Installs the handler for `endpoint`'s files; a second call changes nothing.
pub(crate) fn remove_files_on_termination(endpoint: &DaemonEndpoint) {
    let socket = CString::new(endpoint.socket().into_os_string().into_vec());
    let state = CString::new(endpoint.state().into_os_string().into_vec());
    let (Ok(socket), Ok(state)) = (socket, state) else {
        return;
    };
    if PATHS.set([socket, state]).is_err() {
        return;
    }
    for signal in SIGNALS {
        install(signal);
    }
}

fn install(signal: libc::c_int) {
    let handler: extern "C" fn(libc::c_int) = on_termination;
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = handler as libc::sighandler_t;
        libc::sigemptyset(&raw mut action.sa_mask);
        libc::sigaction(signal, &raw const action, std::ptr::null_mut());
    }
}

extern "C" fn on_termination(signal: libc::c_int) {
    if let Some(paths) = PATHS.get() {
        for path in paths {
            unsafe { libc::unlink(path.as_ptr()) };
        }
    }
    unsafe {
        libc::signal(signal, libc::SIG_DFL);
        libc::raise(signal);
    }
}
