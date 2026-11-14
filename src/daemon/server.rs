//! The daemon process: started by a client with `AGENT_DESKTOP_DAEMON_SERVE=<socket>`, it
//! detaches into its own session, takes the endpoint's lock, binds the socket 0600 inside the
//! private daemon directory and serves until it has been idle for the configured time.

use std::fs::File;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::mpsc::{Sender, channel};
use std::time::Duration;

use super::build_identity::BuildIdentity;
use super::connection::serve_connection;
use super::endpoint::DaemonEndpoint;
use super::env_subset::EnvSubset;
use super::executor::run_until_idle;
use super::job::Job;
use super::server_state::ServerState;
use super::state_file::DaemonStateFile;
use super::{IDLE_ENV, SERVE_ENV};

const DEFAULT_IDLE_MS: u64 = 300_000;
const IDLE_RANGE_MS: std::ops::RangeInclusive<u64> = 100..=86_400_000;

/// Serves when this process was started as a daemon. The variable is removed first, while the
/// process is still single-threaded, so nothing this daemon starts inherits it.
pub(crate) fn serve_from_env() -> Option<ExitCode> {
    let socket = std::env::var_os(SERVE_ENV)?;
    unsafe { std::env::remove_var(SERVE_ENV) };
    Some(serve(Path::new(&socket)))
}

fn serve(socket: &Path) -> ExitCode {
    unsafe { libc::setsid() };
    close_inherited_descriptors();
    let Some(endpoint) = DaemonEndpoint::from_socket(socket).filter(DaemonEndpoint::is_private)
    else {
        return ExitCode::FAILURE;
    };
    let Some(lock) = take_lock(&endpoint.lock()) else {
        return ExitCode::SUCCESS;
    };
    let (Some(build), Some(env)) = (BuildIdentity::current(), EnvSubset::current()) else {
        return ExitCode::FAILURE;
    };
    let Ok(listener) = bind(&endpoint.socket()) else {
        return ExitCode::FAILURE;
    };
    let Ok(record) = DaemonStateFile::create(&endpoint.state()) else {
        let _ = std::fs::remove_file(endpoint.socket());
        return ExitCode::FAILURE;
    };
    let _ = std::env::set_current_dir("/");
    crate::init_tracing(false);
    let server = Arc::new(ServerState::new(endpoint, build, env));
    let (jobs, queue) = channel();
    let accepting = Arc::clone(&server);
    let started = std::thread::Builder::new()
        .name("daemon-accept".to_owned())
        .spawn(move || accept(&listener, &accepting, &jobs));
    if started.is_ok() {
        run_until_idle(&queue, &server, &record, idle_timeout());
    }
    let _ = std::fs::remove_file(server.endpoint.socket());
    record.remove();
    drop(lock);
    ExitCode::SUCCESS
}

fn accept(listener: &UnixListener, server: &Arc<ServerState>, jobs: &Sender<Job>) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else {
            continue;
        };
        if !server.open_connection() {
            continue;
        }
        let held = Arc::clone(server);
        let jobs = jobs.clone();
        let spawned = std::thread::Builder::new()
            .name("daemon-connection".to_owned())
            .spawn(move || {
                serve_connection(stream, &held, &jobs);
                held.close_connection();
            });
        if spawned.is_err() {
            server.close_connection();
        }
    }
}

/// The endpoint's lock, held for the daemon's whole life; `None` when another daemon holds it.
fn take_lock(path: &Path) -> Option<File> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .ok()?;
    let locked = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0;
    locked.then_some(file)
}

/// Binds `path` after removing a socket a dead daemon left there. Only the lock holder gets
/// here, so the socket it removes cannot belong to a live daemon.
fn bind(path: &Path) -> std::io::Result<UnixListener> {
    if let Ok(metadata) = std::fs::symlink_metadata(path) {
        if !metadata.file_type().is_socket() {
            return Err(std::io::Error::other("not a socket"));
        }
        std::fs::remove_file(path)?;
    }
    let listener = UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

fn idle_timeout() -> Duration {
    let millis = std::env::var(IDLE_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|millis| IDLE_RANGE_MS.contains(millis))
        .unwrap_or(DEFAULT_IDLE_MS);
    Duration::from_millis(millis)
}

/// Closes every descriptor above stderr the starting client may have leaked, so the daemon
/// never keeps a caller's pipe or lock open.
fn close_inherited_descriptors() {
    let Ok(entries) = std::fs::read_dir("/dev/fd") else {
        return;
    };
    let descriptors: Vec<i32> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str()?.parse().ok())
        .filter(|fd| *fd > 2)
        .collect();
    for fd in descriptors {
        unsafe { libc::close(fd) };
    }
}
