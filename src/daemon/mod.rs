//! Opt-in persistent-process mode.
//!
//! With `AGENT_DESKTOP_DAEMON=1` or `--daemon` the CLI hands each command to a background
//! process of the same binary that keeps the platform's connections warm between calls. One
//! daemon serves one user, one binary build, one session and agent id, and one set of display,
//! bus and `AGENT_DESKTOP_*` variables; its endpoint lives in a 0700 directory under the state
//! root and it refuses peers of another user. The CLI runs the command in its own process
//! whenever the daemon cannot take it before it starts: the daemon cannot be started or reached
//! in time, is busy, or the command needs this process (stdin, `launch`, `--verbose`, help). A
//! command the daemon started is never run a second time unless it only reads.
//! The OS endpoint sits behind `transport::DaemonTransport`; Unix sockets implement it. On a
//! platform without an implementation (Windows) the CLI always runs commands in its own process.

use std::process::ExitCode;

#[cfg(unix)]
mod admission;
pub(crate) mod args;
#[cfg(unix)]
mod build_identity;
#[cfg(unix)]
mod capture;
#[cfg(unix)]
mod client;
#[cfg(unix)]
mod clock;
#[cfg(unix)]
mod connection;
#[cfg(unix)]
mod endpoint;
#[cfg(unix)]
mod env_subset;
#[cfg(unix)]
mod exchange_outcome;
#[cfg(unix)]
mod executor;
#[cfg(unix)]
mod idle_policy;
#[cfg(unix)]
mod idle_timer;
#[cfg(unix)]
mod in_doubt;
#[cfg(unix)]
mod job;
#[cfg(unix)]
mod job_state;
#[cfg(all(unix, any(target_os = "macos", test)))]
mod macos_permissions;
mod opt_in;
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
mod termination;
#[cfg(unix)]
mod transport;
#[cfg(unix)]
mod unix_listener;
#[cfg(unix)]
mod unix_transport;
#[cfg(unix)]
mod wire;

/// Set to `1` to run every command through the daemon; `--daemon` does it for one call.
pub(crate) const OPT_IN_ENV: &str = "AGENT_DESKTOP_DAEMON";
#[cfg(unix)]
/// Set by the CLI on the process it starts; names the socket that process serves.
pub(crate) const SERVE_ENV: &str = "AGENT_DESKTOP_DAEMON_SERVE";
#[cfg(unix)]
/// How long an idle daemon waits for its next command before it exits, in milliseconds.
pub(crate) const IDLE_ENV: &str = "AGENT_DESKTOP_DAEMON_IDLE_MS";

/// Serves commands when this process was started as a daemon; `None` for every other process.
pub(crate) fn serve_if_requested() -> Option<ExitCode> {
    #[cfg(unix)]
    {
        server::serve_from_env(&unix_transport::UnixTransport)
    }
    #[cfg(not(unix))]
    {
        None
    }
}

/// Runs this invocation through the daemon when daemon mode is on and the daemon takes it.
/// `None` means the caller runs the command in this process, exactly as without daemon mode.
pub(crate) fn run_through_daemon() -> Option<ExitCode> {
    if !opt_in::requested() {
        return None;
    }
    #[cfg(unix)]
    {
        client::run(&unix_transport::UnixTransport)
    }
    #[cfg(not(unix))]
    {
        None
    }
}
