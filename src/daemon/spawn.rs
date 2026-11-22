//! Starts a daemon for an endpoint. The client that starts it then waits a bounded time for it
//! to bind, so the first call of a session already runs in the daemon.

use std::os::fd::OwnedFd;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

use super::endpoint::DaemonEndpoint;
use super::private_dir::ensure_private_dir;
use super::startup::launch_socket;
use super::{OPT_IN_ENV, SERVE_ENV};

/// Starts this binary as the daemon for `endpoint`, in a new session with no terminal and no
/// standard streams; `false` when it could not be started, so this call runs in its own process.
pub(crate) fn start_daemon(endpoint: &DaemonEndpoint) -> bool {
    if ensure_private_dir(endpoint.dir()).is_err() {
        return false;
    }
    let Some(executable) = own_executable() else {
        return false;
    };
    let Ok(launch) = launch_socket() else {
        return false;
    };
    let mut command = Command::new(executable);
    command
        .env(OPT_IN_ENV, "1")
        .env(SERVE_ENV, endpoint.socket())
        .current_dir("/")
        .stdin(Stdio::from(OwnedFd::from(launch)))
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    unsafe {
        command.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    command.spawn().is_ok()
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
