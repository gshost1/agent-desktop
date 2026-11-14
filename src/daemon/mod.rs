//! Opt-in persistent-process mode.
//!
//! With `AGENT_DESKTOP_DAEMON=1` the CLI hands each command to a background process of the same
//! binary that keeps the platform's connections warm between calls. One daemon serves one user,
//! one binary build, one session and agent id, and one set of display, bus and `AGENT_DESKTOP_*`
//! variables; its socket lives in a 0700 directory under the state root and it refuses peers of
//! another user. The CLI runs the command in its own process whenever the daemon cannot take it
//! before dispatch: no daemon yet (it starts one for the next call), busy, another build or
//! environment, or a command that needs this process (stdin, `launch`, `--verbose`, help). A
//! command the daemon accepted is never run a second time unless it only reads.
//! Unix only: on Windows the CLI always runs commands in its own process.

use std::process::ExitCode;

#[cfg(unix)]
mod admission;
#[cfg(unix)]
mod build_identity;
#[cfg(unix)]
mod capture;
#[cfg(unix)]
mod client;
#[cfg(unix)]
mod connection;
#[cfg(unix)]
mod endpoint;
#[cfg(unix)]
mod env_subset;
#[cfg(unix)]
mod executor;
#[cfg(unix)]
mod in_doubt;
#[cfg(unix)]
mod job;
#[cfg(unix)]
mod job_state;
#[cfg(unix)]
mod peer;
#[cfg(unix)]
mod private_dir;
#[cfg(unix)]
mod reap;
#[cfg(unix)]
mod reply;
#[cfg(unix)]
mod request;
#[cfg(unix)]
mod rerun;
#[cfg(unix)]
mod server;
#[cfg(unix)]
mod server_state;
#[cfg(unix)]
mod spawn;
#[cfg(unix)]
mod state_file;
#[cfg(unix)]
mod wire;

/// Set to `1` to run commands through the daemon.
pub(crate) const OPT_IN_ENV: &str = "AGENT_DESKTOP_DAEMON";
/// Set by the CLI on the process it starts; names the socket that process serves.
pub(crate) const SERVE_ENV: &str = "AGENT_DESKTOP_DAEMON_SERVE";
/// How long an idle daemon waits for its next command before it exits, in milliseconds.
pub(crate) const IDLE_ENV: &str = "AGENT_DESKTOP_DAEMON_IDLE_MS";

/// Serves commands when this process was started as a daemon; `None` for every other process.
pub(crate) fn serve_if_requested() -> Option<ExitCode> {
    #[cfg(unix)]
    {
        server::serve_from_env()
    }
    #[cfg(not(unix))]
    {
        None
    }
}

/// Runs this invocation through the daemon when daemon mode is on and the daemon takes it.
/// `None` means the caller runs the command in this process, exactly as without daemon mode.
pub(crate) fn run_through_daemon() -> Option<ExitCode> {
    #[cfg(unix)]
    {
        client::run()
    }
    #[cfg(not(unix))]
    {
        None
    }
}
