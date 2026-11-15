//! Runs admitted commands one at a time on the daemon's main thread, the thread a one-shot CLI
//! runs them on, through the same `run_args` path, with the client's working directory and
//! file-creation mask, and its stdout and stderr captured.

use std::ffi::OsString;
use std::process::ExitCode;
use std::sync::PoisonError;
use std::sync::mpsc::{Receiver, RecvTimeoutError};

use super::capture::{CapturedOutput, OutputCapture};
use super::clock::Clock;
use super::idle_policy::IdlePolicy;
use super::idle_timer::IdleTimer;
use super::job::{Job, SharedWriter};
use super::reap::reap_exited_children;
use super::reply::DaemonReply;
use super::request::DaemonRequest;
use super::rerun::is_undelivered_permission_refusal;
use super::server_state::ServerState;
use super::state_file::DaemonStateFile;
use super::wire::{write_frame, write_message};

/// Serves jobs until none arrives for the policy's idle time on `clock`, then claims the
/// execution slot for good so no connection can hand over another command while it exits.
pub(crate) fn run_until_idle(
    jobs: &Receiver<Job>,
    server: &ServerState,
    record: &DaemonStateFile,
    policy: IdlePolicy,
    clock: &dyn Clock,
) {
    let mut served = 0_u64;
    let mut timer = IdleTimer::start(policy, clock.now());
    loop {
        match jobs.recv_timeout(policy.tick) {
            Ok(job) => {
                let finished = run_job(job, server);
                if finished.is_some() {
                    served += 1;
                    let _ = record.record(served);
                }
                if let Some((writer, exit, output)) = finished {
                    reply(&writer, exit, output);
                }
                server.release();
                timer.touch(clock.now());
            }
            Err(RecvTimeoutError::Timeout) => {
                if timer.expired(clock.now()) && server.claim() {
                    return;
                }
            }
            Err(RecvTimeoutError::Disconnected) => return,
        }
        reap_exited_children();
    }
}

/// Runs one job and returns its result for the client, or `None` when it did not run. The
/// caller counts it before it replies, so a client that reads the state file after its answer
/// sees its call counted, and frees the slot after the reply, so no new command is committed
/// while a reply to a stalled client is still being written.
fn run_job(job: Job, server: &ServerState) -> Option<(SharedWriter, u8, CapturedOutput)> {
    let Job {
        writer,
        request,
        state,
    } = job;
    if !state.start() {
        return None;
    }
    let finished = run_in_client_context(&writer, &request, server);
    state.finish();
    let (exit, output) = finished?;
    Some((writer, exit, output))
}

/// Runs the command from the client's directory with its file-creation mask, then puts the
/// daemon's own back. A directory the daemon cannot enter ends the connection unanswered before
/// `Started`, so the client runs the command itself.
fn run_in_client_context(
    writer: &SharedWriter,
    request: &DaemonRequest,
    server: &ServerState,
) -> Option<(u8, CapturedOutput)> {
    std::env::set_current_dir(&request.cwd).ok()?;
    let previous_mask = set_umask(request.umask);
    let finished = OutputCapture::begin(&server.endpoint)
        .ok()
        .and_then(|capture| run_captured(capture, writer, &request.argv));
    set_umask(previous_mask);
    let _ = std::env::set_current_dir("/");
    finished
}

fn run_captured(
    capture: OutputCapture,
    writer: &SharedWriter,
    argv: &[String],
) -> Option<(u8, CapturedOutput)> {
    if send(writer, &DaemonReply::Started).is_err() {
        let _ = capture.finish();
        return None;
    }
    let args: Vec<OsString> = argv.iter().map(OsString::from).collect();
    let exit = exit_byte(crate::run_args(&args));
    #[cfg(target_os = "linux")]
    agent_desktop_linux::end_input_session();
    let output = capture.finish().ok()?;
    Some((exit, output))
}

fn send(writer: &SharedWriter, reply: &DaemonReply) -> std::io::Result<()> {
    let mut stream = writer.lock().unwrap_or_else(PoisonError::into_inner);
    write_message(&mut *stream, reply)
}

/// Sends the result in one locked write, so no heartbeat lands between its three frames. A
/// client that left gets nothing; its command already ran, as a killed CLI's would have.
fn reply(writer: &SharedWriter, exit: u8, (stdout, stderr): CapturedOutput) {
    let mut stream = writer.lock().unwrap_or_else(PoisonError::into_inner);
    if exit == 1 && is_undelivered_permission_refusal(&stdout) {
        let rerun = DaemonReply::Rerun {
            reason: "permission".to_owned(),
        };
        let _ = write_message(&mut *stream, &rerun);
        return;
    }
    let _ = write_message(&mut *stream, &DaemonReply::Done { exit })
        .and_then(|()| write_frame(&mut *stream, &stdout))
        .and_then(|()| write_frame(&mut *stream, &stderr));
}

#[allow(clippy::useless_conversion)]
fn set_umask(mask: u32) -> u32 {
    let mask = libc::mode_t::try_from(mask & 0o777).unwrap_or(0o022);
    u32::from(unsafe { libc::umask(mask) })
}

/// The process exit status `code` stands for. `run_args` ends with success, failure or the
/// argument-error status, so those are the only values to tell apart.
fn exit_byte(code: ExitCode) -> u8 {
    if code == ExitCode::SUCCESS {
        0
    } else if code == ExitCode::from(crate::EXIT_INVALID_ARGS) {
        crate::EXIT_INVALID_ARGS
    } else {
        1
    }
}

#[cfg(test)]
#[path = "executor_tests.rs"]
mod tests;
