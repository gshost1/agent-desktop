//! One client connection: check its request, admit the command or decline it, take the execution
//! slot, offer `Ready` and wait for the client's go, hand the command to the executor, then send
//! heartbeats and watch for the client going away while the command is queued or running.

use std::io::{ErrorKind, Read};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use super::admission::admit;
use super::job::{Job, SharedWriter};
use super::job_state::JobState;
use super::reply::{DaemonReply, GO};
use super::request::DaemonRequest;
use super::server_state::ServerState;
use super::transport::DaemonStream;
use super::wire::{REQUEST_LIMIT, read_message, write_message};

const REQUEST_READ_TIMEOUT: Duration = Duration::from_secs(5);
/// How long the slot stays held for a client that was offered `Ready` and has not said go.
const GO_TIMEOUT: Duration = Duration::from_secs(2);
/// How often a queued or running command's client hears that the daemon is alive.
pub(crate) const HEARTBEAT: Duration = Duration::from_millis(500);
const STOP_GRACE: Duration = Duration::from_secs(5);
/// How long a reply or heartbeat may wait on a client that stopped reading. The bound keeps a
/// stalled client from holding the executor, which serves the next caller.
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);

/// Serves one accepted connection to its end.
pub(crate) fn serve_connection(
    stream: Box<dyn DaemonStream>,
    server: &ServerState,
    jobs: &Sender<Job>,
) {
    let Some((mut stream, request)) = read_request(stream) else {
        return;
    };
    if let Some(reason) = server.mismatch(&request) {
        decline(stream.as_mut(), reason);
        return;
    }
    let admitted = match admit(&request.argv) {
        Ok(admitted) => admitted,
        Err(reason) => {
            decline(stream.as_mut(), reason);
            return;
        }
    };
    if !server.claim() {
        decline(stream.as_mut(), "busy");
        return;
    }
    let ready = DaemonReply::Ready {
        command: admitted.command.to_owned(),
        rerun_safe: admitted.rerun_safe,
    };
    if write_message(&mut stream, &ready).is_err() || !received_go(stream.as_mut()) {
        server.release();
        return;
    }
    let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));
    let Ok(watch) = stream.try_clone_stream() else {
        server.release();
        return;
    };
    let state = Arc::new(JobState::default());
    let writer: SharedWriter = Arc::new(Mutex::new(stream));
    let job = Job {
        writer: Arc::clone(&writer),
        request,
        state: Arc::clone(&state),
    };
    if jobs.send(job).is_err() {
        server.release();
        return;
    }
    watch_until_settled(watch, &writer, &state);
}

fn read_request(
    mut stream: Box<dyn DaemonStream>,
) -> Option<(Box<dyn DaemonStream>, DaemonRequest)> {
    let _ = stream.set_read_timeout(Some(REQUEST_READ_TIMEOUT));
    let request = read_message::<DaemonRequest>(&mut stream, REQUEST_LIMIT).ok()?;
    Some((stream, request))
}

/// Whether the client said go within `GO_TIMEOUT`. A client that hangs up, stalls or sends
/// anything else has not committed, so the command never runs here. A failed timeout change
/// never discards a frame: macOS refuses `SO_RCVTIMEO` with `EINVAL` once the peer has closed,
/// while a go it sent before closing is still readable, and a read from a closed peer cannot
/// block.
fn received_go(stream: &mut dyn DaemonStream) -> bool {
    let _ = stream.set_read_timeout(Some(GO_TIMEOUT));
    let mut reader = &mut *stream;
    let go = read_message::<String>(&mut reader, REQUEST_LIMIT).is_ok_and(|word| word == GO);
    let _ = stream.set_read_timeout(None);
    go
}

pub(crate) fn decline(stream: &mut dyn DaemonStream, reason: &str) {
    let mut writer = stream;
    let _ = write_message(
        &mut writer,
        &DaemonReply::Declined {
            reason: reason.to_owned(),
        },
    );
}

/// A client sends nothing after its go, so a read returns only when it closes. Between reads
/// the client gets a heartbeat even if its command is still queued. A client that vanishes while
/// its command runs stops the daemon, as killing a one-shot CLI stops the command it was running. Where
/// the heartbeat timeout cannot be set (macOS, after the peer closed) the read blocks until the
/// hangup it then reports at once.
fn watch_until_settled(mut watch: Box<dyn DaemonStream>, writer: &SharedWriter, state: &JobState) {
    let _ = watch.set_read_timeout(Some(HEARTBEAT));
    let mut byte = [0_u8; 1];
    loop {
        match watch.read(&mut byte) {
            Ok(0) => break,
            Ok(_) => {}
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                if state.is_settled() {
                    return;
                }
                send_heartbeat(writer);
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
    if state.client_left() {
        stop_daemon();
    }
}

fn send_heartbeat(writer: &SharedWriter) {
    let mut stream = writer.lock().unwrap_or_else(PoisonError::into_inner);
    let _ = write_message(&mut *stream, &DaemonReply::Running);
}

fn stop_daemon() {
    unsafe { libc::kill(libc::getpid(), libc::SIGTERM) };
    std::thread::sleep(STOP_GRACE);
    unsafe { libc::_exit(1) };
}

#[cfg(test)]
#[path = "connection_tests.rs"]
mod tests;
