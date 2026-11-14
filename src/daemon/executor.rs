//! Runs admitted commands one at a time on the daemon's main thread, the thread a one-shot CLI
//! runs them on, through the same `run_args` path, with the client's working directory and
//! file-creation mask, and its stdout and stderr captured.

use std::ffi::OsString;
use std::os::unix::net::UnixStream;
use std::process::ExitCode;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use super::capture::{CapturedOutput, OutputCapture};
use super::connection::decline;
use super::job::Job;
use super::reap::reap_exited_children;
use super::reply::DaemonReply;
use super::rerun::is_undelivered_permission_refusal;
use super::server_state::ServerState;
use super::state_file::DaemonStateFile;
use super::wire::{write_frame, write_message};

const TICK: Duration = Duration::from_secs(1);

/// Serves jobs until none arrives for `idle`, then claims the execution slot for good so no
/// connection can hand over another command while the daemon exits.
pub(crate) fn run_until_idle(
    jobs: &Receiver<Job>,
    server: &ServerState,
    record: &DaemonStateFile,
    idle: Duration,
) {
    let mut served = 0_u64;
    let mut last_activity = Instant::now();
    loop {
        match jobs.recv_timeout(TICK.min(idle)) {
            Ok(job) => {
                if run_job(job, server) {
                    served += 1;
                    let _ = record.record(served);
                }
                server.release();
                last_activity = Instant::now();
            }
            Err(RecvTimeoutError::Timeout) => {
                if last_activity.elapsed() >= idle && server.claim() {
                    return;
                }
            }
            Err(RecvTimeoutError::Disconnected) => return,
        }
        reap_exited_children();
    }
}

/// Runs one job; `true` when its command ran and its result reached the client.
fn run_job(job: Job, server: &ServerState) -> bool {
    let Job {
        stream,
        request,
        admitted,
        state,
    } = job;
    if std::env::set_current_dir(&request.cwd).is_err() {
        decline(stream, "cwd");
        return false;
    }
    let previous_mask = set_umask(request.umask);
    let finished = match OutputCapture::begin(&server.endpoint) {
        Ok(capture) => {
            let accepted = DaemonReply::Accepted {
                command: admitted.command.to_owned(),
                rerun_safe: admitted.rerun_safe,
            };
            run_captured(capture, stream, &accepted, &request.argv, &state)
        }
        Err(_) => {
            decline(stream, "capture");
            None
        }
    };
    set_umask(previous_mask);
    let _ = std::env::set_current_dir("/");
    state.finish();
    finished.is_some_and(|(stream, exit, output)| reply(stream, exit, output))
}

fn run_captured(
    capture: OutputCapture,
    mut stream: UnixStream,
    accepted: &DaemonReply,
    argv: &[String],
    state: &super::job_state::JobState,
) -> Option<(UnixStream, u8, CapturedOutput)> {
    if !state.start() || write_message(&mut stream, accepted).is_err() {
        let _ = capture.finish();
        return None;
    }
    let args: Vec<OsString> = argv.iter().map(OsString::from).collect();
    let exit = exit_byte(crate::run_args(&args));
    #[cfg(target_os = "linux")]
    agent_desktop_linux::end_input_session();
    let output = capture.finish().ok()?;
    Some((stream, exit, output))
}

fn reply(mut stream: UnixStream, exit: u8, (stdout, stderr): CapturedOutput) -> bool {
    if exit == 1 && is_undelivered_permission_refusal(&stdout) {
        let rerun = DaemonReply::Rerun {
            reason: "permission".to_owned(),
        };
        let _ = write_message(&mut stream, &rerun);
        return false;
    }
    write_message(&mut stream, &DaemonReply::Done { exit }).is_ok()
        && write_frame(&mut stream, &stdout).is_ok()
        && write_frame(&mut stream, &stderr).is_ok()
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
