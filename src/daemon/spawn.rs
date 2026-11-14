//! Starts a daemon for an endpoint without waiting for it: the call that starts it runs its own
//! command in its own process, and the next call finds the daemon ready.

use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

use super::SERVE_ENV;
use super::endpoint::DaemonEndpoint;
use super::private_dir::ensure_private_dir;

/// Starts this binary as the daemon for `endpoint`, in a new session with no terminal and no
/// standard streams. Failure only means the next call runs in its own process too.
pub(crate) fn start_daemon(endpoint: &DaemonEndpoint) {
    if ensure_private_dir(endpoint.dir()).is_err() {
        return;
    }
    let Some(executable) = own_executable() else {
        return;
    };
    let mut command = Command::new(executable);
    command
        .env(SERVE_ENV, endpoint.socket())
        .current_dir("/")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    unsafe {
        command.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    let _ = command.spawn();
}

/// The file to start. On Linux `/proc/self/exe` is the very inode this client runs, so a binary
/// replaced on disk since this client started never becomes its daemon.
fn own_executable() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "linux")]
    {
        Some(std::path::PathBuf::from("/proc/self/exe"))
    }
    #[cfg(not(target_os = "linux"))]
    {
        std::env::current_exe().ok()
    }
}
