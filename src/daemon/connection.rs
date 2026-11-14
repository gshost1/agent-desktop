//! One client connection: check the peer and its request, admit the command or decline it, hand
//! it to the executor, then watch for the client going away while it runs.

use std::io::Read;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::Duration;

use super::admission::admit;
use super::job::Job;
use super::job_state::JobState;
use super::peer::is_trusted_peer;
use super::reply::DaemonReply;
use super::request::DaemonRequest;
use super::server_state::ServerState;
use super::wire::{REQUEST_LIMIT, read_message, write_message};

const REQUEST_READ_TIMEOUT: Duration = Duration::from_secs(5);
const STOP_GRACE: Duration = Duration::from_secs(5);

/// Serves one accepted connection to its end.
pub(crate) fn serve_connection(stream: UnixStream, server: &ServerState, jobs: &Sender<Job>) {
    if !is_trusted_peer(&stream) {
        return;
    }
    let Some((stream, request)) = read_request(stream) else {
        return;
    };
    if let Some(reason) = server.mismatch(&request) {
        decline(stream, reason);
        return;
    }
    let admitted = match admit(&request.argv) {
        Ok(admitted) => admitted,
        Err(reason) => {
            decline(stream, reason);
            return;
        }
    };
    if !server.claim() {
        decline(stream, "busy");
        return;
    }
    let Ok(watch) = stream.try_clone() else {
        server.release();
        decline(stream, "connection");
        return;
    };
    let state = Arc::new(JobState::default());
    let job = Job {
        stream,
        request,
        admitted,
        state: Arc::clone(&state),
    };
    if jobs.send(job).is_err() {
        server.release();
        return;
    }
    watch_for_hangup(watch, &state);
}

fn read_request(mut stream: UnixStream) -> Option<(UnixStream, DaemonRequest)> {
    stream.set_read_timeout(Some(REQUEST_READ_TIMEOUT)).ok()?;
    let request = read_message::<DaemonRequest>(&mut stream, REQUEST_LIMIT).ok()?;
    stream.set_read_timeout(None).ok()?;
    Some((stream, request))
}

pub(crate) fn decline(mut stream: UnixStream, reason: &str) {
    let _ = write_message(
        &mut stream,
        &DaemonReply::Declined {
            reason: reason.to_owned(),
        },
    );
}

/// A client sends nothing after its request, so a read returns only when it closes. A client
/// that vanishes while its command runs stops the daemon, exactly as killing a one-shot CLI
/// stops the command it was running.
fn watch_for_hangup(mut stream: UnixStream, state: &JobState) {
    let mut byte = [0_u8; 1];
    loop {
        match stream.read(&mut byte) {
            Ok(0) => break,
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
    if state.client_left() {
        stop_daemon();
    }
}

fn stop_daemon() {
    unsafe { libc::kill(libc::getpid(), libc::SIGTERM) };
    std::thread::sleep(STOP_GRACE);
    std::process::exit(1);
}
