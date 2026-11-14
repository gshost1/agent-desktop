//! The client half: send this invocation to its daemon and reproduce the daemon's stdout,
//! stderr and exit status, or return `None` so the caller runs the command in this process.
//! Every failure before the daemon accepts the command returns `None`; after it accepts, only
//! a read-only command may run again here.

use std::io::{ErrorKind, Write};
use std::os::unix::net::UnixStream;
use std::process::ExitCode;

use super::OPT_IN_ENV;
use super::endpoint::DaemonEndpoint;
use super::in_doubt::daemon_stopped_mid_command;
use super::reply::DaemonReply;
use super::request::DaemonRequest;
use super::spawn::start_daemon;
use super::wire::{REPLY_LIMIT, read_frame, read_message, write_message};

/// Runs this invocation through its daemon, starting one for the next call when none answers.
pub(crate) fn run() -> Option<ExitCode> {
    if std::env::var_os(OPT_IN_ENV).is_none_or(|value| value != "1") {
        return None;
    }
    if inherits_interaction_lease() {
        return None;
    }
    let request = DaemonRequest::current()?;
    let endpoint = DaemonEndpoint::for_request(&request)?;
    let stream = connect(&endpoint)?;
    exchange(stream, &request)
}

/// An MCP server hands its child the interaction lease as a descriptor or handle, which only
/// this process holds, so such a call always runs here.
fn inherits_interaction_lease() -> bool {
    [
        agent_desktop_core::INTERACTION_LEASE_FD_ENV,
        agent_desktop_core::INTERACTION_LEASE_HANDLE_ENV,
    ]
    .into_iter()
    .any(|name| std::env::var_os(name).is_some())
}

fn connect(endpoint: &DaemonEndpoint) -> Option<UnixStream> {
    if let Err(error) = super::private_dir::check_private_dir(endpoint.dir()) {
        if error.kind() == ErrorKind::NotFound {
            start_daemon(endpoint);
        }
        return None;
    }
    match UnixStream::connect(endpoint.socket()) {
        Ok(stream) => Some(stream),
        Err(error) if matches!(error.kind(), ErrorKind::NotFound | ErrorKind::ConnectionRefused) => {
            start_daemon(endpoint);
            None
        }
        Err(_) => None,
    }
}

/// Sends `request` over `stream` and turns the daemon's answer into this process's result.
pub(crate) fn exchange(mut stream: UnixStream, request: &DaemonRequest) -> Option<ExitCode> {
    write_message(&mut stream, request).ok()?;
    let DaemonReply::Accepted {
        command,
        rerun_safe,
    } = read_message::<DaemonReply>(&mut stream, REPLY_LIMIT).ok()?
    else {
        return None;
    };
    match read_result(&mut stream) {
        Ok(Some((exit, stdout, stderr))) => Some(reproduce(exit, &stdout, &stderr)),
        Ok(None) => None,
        Err(_) if rerun_safe => None,
        Err(_) => Some(crate::cli::emit::finish(
            &command,
            Err(daemon_stopped_mid_command()),
            false,
        )),
    }
}

/// The finished command's status and output, `None` for a `Rerun`, or an error when the daemon
/// stopped answering before the result was complete.
fn read_result(stream: &mut UnixStream) -> std::io::Result<Option<(u8, Vec<u8>, Vec<u8>)>> {
    match read_message::<DaemonReply>(stream, REPLY_LIMIT)? {
        DaemonReply::Done { exit } => {
            let stdout = read_frame(stream, REPLY_LIMIT)?;
            let stderr = read_frame(stream, REPLY_LIMIT)?;
            Ok(Some((exit, stdout, stderr)))
        }
        DaemonReply::Rerun { .. } => Ok(None),
        DaemonReply::Declined { .. } | DaemonReply::Accepted { .. } => {
            Err(std::io::Error::other("unexpected reply after acceptance"))
        }
    }
}

/// Writes the daemon's stderr, then its stdout, as the command wrote them, and exits as it did.
fn reproduce(exit: u8, stdout: &[u8], stderr: &[u8]) -> ExitCode {
    let mut err = std::io::stderr().lock();
    let _ = err.write_all(stderr).and_then(|()| err.flush());
    let mut out = std::io::stdout().lock();
    match out.write_all(stdout).and_then(|()| out.flush()) {
        Ok(()) => ExitCode::from(exit),
        Err(error) => crate::cli::emit::report_output_failure(error),
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
