//! The client half: send this invocation to its daemon and reproduce the daemon's stdout,
//! stderr and exit status, or return `None` so the caller runs the command in this process.
//! Until the daemon says `Started` the command has not run there, so every failure up to then
//! returns `None`; after it, only a read-only command may run again here.

use std::io::{ErrorKind, Write};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use super::endpoint::DaemonEndpoint;
use super::exchange_outcome::Outcome;
use super::in_doubt::daemon_stopped_mid_command;
use super::reply::{DaemonReply, GO};
use super::request::DaemonRequest;
use super::spawn::start_daemon;
use super::transport::{DaemonStream, DaemonTransport};
use super::wire::{REPLY_LIMIT, read_frame, read_message, write_message};

/// How long a client waits for a daemon it started to bind its endpoint.
const START_WAIT: Duration = Duration::from_millis(1500);
/// How long a client waits for `Ready` or `Declined`; a daemon that says neither is hung.
const READY_TIMEOUT: Duration = Duration::from_secs(2);
/// How long a running command may go without a heartbeat before its daemon counts as gone.
const STALL_LIMIT: Duration = Duration::from_secs(5);

/// Runs this invocation through its daemon, starting one when none answers.
pub(crate) fn run(transport: &dyn DaemonTransport) -> Option<ExitCode> {
    let request = DaemonRequest::current()?;
    #[cfg(target_os = "macos")]
    if !super::macos_permissions::permits_forwarding(
        &request.argv,
        std::env::var_os("AGENT_DESKTOP_SESSION").is_some(),
        agent_desktop_macos::preflight_accessibility,
        agent_desktop_macos::preflight_screen_recording,
    ) {
        return None;
    }
    let endpoint = DaemonEndpoint::for_request(&request)?;
    let stream = connect(transport, &endpoint)?;
    exchange(stream, &request)
}

fn connect(
    transport: &dyn DaemonTransport,
    endpoint: &DaemonEndpoint,
) -> Option<Box<dyn DaemonStream>> {
    let private = super::private_dir::check_private_dir(endpoint.dir());
    if let Err(error) = &private
        && error.kind() != ErrorKind::NotFound
    {
        return None;
    }
    if private.is_ok() {
        match transport.connect(endpoint) {
            Ok(stream) => return Some(stream),
            Err(error) if !transport.is_absent(&error) => return None,
            Err(_) => {}
        }
    }
    if !start_daemon(endpoint) {
        return None;
    }
    connect_when_bound(transport, endpoint)
}

/// Retries the connection with a growing pause until the started daemon binds or
/// `START_WAIT` passes.
fn connect_when_bound(
    transport: &dyn DaemonTransport,
    endpoint: &DaemonEndpoint,
) -> Option<Box<dyn DaemonStream>> {
    let started = Instant::now();
    let mut pause = Duration::from_millis(1);
    while started.elapsed() < START_WAIT {
        std::thread::sleep(pause);
        match transport.connect(endpoint) {
            Ok(stream) => return Some(stream),
            Err(error) if !transport.is_absent(&error) => return None,
            Err(_) => pause = (pause * 2).min(Duration::from_millis(20)),
        }
    }
    None
}

/// Sends `request` over `stream` and turns the daemon's answer into this process's result.
pub(crate) fn exchange(
    mut stream: Box<dyn DaemonStream>,
    request: &DaemonRequest,
) -> Option<ExitCode> {
    stream.set_read_timeout(Some(READY_TIMEOUT)).ok()?;
    write_message(&mut stream, request).ok()?;
    let DaemonReply::Ready {
        command,
        rerun_safe,
    } = read_message::<DaemonReply>(&mut stream, REPLY_LIMIT).ok()?
    else {
        return None;
    };
    stream.set_read_timeout(Some(STALL_LIMIT)).ok()?;
    write_message(&mut stream, &GO).ok()?;
    match read_result(stream.as_mut()) {
        Outcome::Finished {
            exit,
            stdout,
            stderr,
        } => Some(reproduce(exit, &stdout, &stderr)),
        Outcome::NotRun => None,
        Outcome::Lost if rerun_safe => None,
        Outcome::Lost => Some(crate::cli::emit::finish(
            &command,
            Err(daemon_stopped_mid_command()),
            false,
        )),
    }
}

/// Reads to the result. Only `Started` proves the command ran: a heartbeat can precede a setup
/// failure that ends the connection unrun. After the go the client is committed, so a daemon
/// that falls silent before `Started` leaves the command in doubt, while one that closes the
/// connection before it proves the command never ran.
fn read_result(mut stream: &mut dyn DaemonStream) -> Outcome {
    let mut started = false;
    loop {
        let reply = match read_message::<DaemonReply>(&mut stream, REPLY_LIMIT) {
            Ok(reply) => reply,
            Err(error) if started || is_timeout(&error) => return Outcome::Lost,
            Err(_) => return Outcome::NotRun,
        };
        match reply {
            DaemonReply::Started => started = true,
            DaemonReply::Running => {}
            DaemonReply::Done { exit } => return read_output(stream, exit),
            DaemonReply::Rerun { .. } => return Outcome::NotRun,
            DaemonReply::Declined { .. } | DaemonReply::Ready { .. } => return Outcome::Lost,
        }
    }
}

fn is_timeout(error: &std::io::Error) -> bool {
    matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)
}

fn read_output(mut stream: &mut dyn DaemonStream, exit: u8) -> Outcome {
    let frames = read_frame(&mut stream, REPLY_LIMIT)
        .and_then(|stdout| Ok((stdout, read_frame(&mut stream, REPLY_LIMIT)?)));
    match frames {
        Ok((stdout, stderr)) => Outcome::Finished {
            exit,
            stdout,
            stderr,
        },
        Err(_) => Outcome::Lost,
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
