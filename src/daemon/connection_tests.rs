use super::serve_connection;
use crate::daemon::build_identity::BuildIdentity;
use crate::daemon::endpoint::DaemonEndpoint;
use crate::daemon::env_subset::EnvSubset;
use crate::daemon::reply::{DaemonReply, GO};
use crate::daemon::request::{DaemonRequest, PROTOCOL_VERSION};
use crate::daemon::server_state::ServerState;
use crate::daemon::transport::DaemonStream;
use crate::daemon::wire::{REPLY_LIMIT, read_message, write_message};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::mpsc::channel;
use std::time::Duration;

fn server() -> ServerState {
    let metadata = std::fs::metadata(std::env::current_exe().unwrap()).unwrap();
    ServerState::new(
        DaemonEndpoint::from_socket(std::path::Path::new("/r/daemon/0123456789abcdef.sock"))
            .unwrap(),
        BuildIdentity::from_parts("9.9.9", &metadata),
        EnvSubset::from_pairs(Vec::new()),
    )
}

fn request() -> DaemonRequest {
    let metadata = std::fs::metadata(std::env::current_exe().unwrap()).unwrap();
    DaemonRequest {
        protocol: PROTOCOL_VERSION,
        build: BuildIdentity::from_parts("9.9.9", &metadata)
            .as_str()
            .to_owned(),
        argv: vec![
            "agent-desktop".into(),
            "click".into(),
            "@s1234abcd:e1".into(),
        ],
        cwd: "/".into(),
        umask: 0o022,
        env: EnvSubset::from_pairs(Vec::new()),
    }
}

/// Serves one connection whose client sends the request, reads the reply, then either says
/// `go` or hangs up. Returns the reply, whether a job reached the executor, and whether the
/// execution slot is free afterwards.
fn serve_with(say_go: bool) -> (DaemonReply, bool, bool) {
    serve_over(say_go, |stream| Box::new(stream))
}

/// A socket whose timeouts cannot be changed, as macOS answers `EINVAL` for a socket whose peer
/// has closed.
struct ClosedPeerTimeouts(UnixStream);

impl Read for ClosedPeerTimeouts {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl Write for ClosedPeerTimeouts {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

impl DaemonStream for ClosedPeerTimeouts {
    fn try_clone_stream(&self) -> std::io::Result<Box<dyn DaemonStream>> {
        Ok(Box::new(Self(self.0.try_clone()?)))
    }

    fn set_read_timeout(&self, _: Option<Duration>) -> std::io::Result<()> {
        Err(std::io::Error::from_raw_os_error(libc::EINVAL))
    }

    fn set_write_timeout(&self, _: Option<Duration>) -> std::io::Result<()> {
        Err(std::io::Error::from_raw_os_error(libc::EINVAL))
    }
}

fn serve_over(
    say_go: bool,
    wrap: impl FnOnce(UnixStream) -> Box<dyn DaemonStream> + Send,
) -> (DaemonReply, bool, bool) {
    let (client, daemon_end) = UnixStream::pair().unwrap();
    let state = server();
    let (jobs, queue) = channel();
    let client = std::thread::spawn(move || {
        let mut client = client;
        write_message(&mut client, &request()).unwrap();
        let reply: DaemonReply = read_message(&mut client, REPLY_LIMIT).unwrap();
        if say_go {
            write_message(&mut client, &GO).unwrap();
        }
        reply
    });
    let reply_thread = client;
    std::thread::scope(|scope| {
        scope.spawn(|| serve_connection(wrap(daemon_end), &state, &jobs));
    });
    let reply = reply_thread.join().unwrap();
    let job = queue.try_recv().ok();
    if let Some(job) = &job {
        job.state.finish();
        state.release();
    }
    let slot_free = state.claim();
    (reply, job.is_some(), slot_free)
}

#[test]
fn a_client_that_hangs_up_after_ready_never_has_its_command_run() {
    let (reply, ran, slot_free) = serve_with(false);

    assert!(
        matches!(
            reply,
            DaemonReply::Ready {
                rerun_safe: false,
                ..
            }
        ),
        "{reply:?}"
    );
    assert!(!ran, "no go, no job");
    assert!(slot_free, "the slot is released for the next client");
}

#[test]
fn a_client_that_says_go_hands_its_command_to_the_executor() {
    let (reply, ran, _) = serve_with(true);

    assert!(matches!(reply, DaemonReply::Ready { .. }), "{reply:?}");
    assert!(ran);
}

#[test]
fn a_busy_daemon_declines_before_offering_ready() {
    let state = server();
    assert!(state.claim());
    let (client, daemon_end) = UnixStream::pair().unwrap();
    let (jobs, queue) = channel();
    let mut client = client;
    write_message(&mut client, &request()).unwrap();
    serve_connection(Box::new(daemon_end), &state, &jobs);
    let reply: DaemonReply = read_message(&mut client, REPLY_LIMIT).unwrap();

    assert_eq!(
        reply,
        DaemonReply::Declined {
            reason: "busy".into()
        }
    );
    assert!(queue.try_recv().is_err());
}

#[test]
fn a_go_sent_before_the_client_closed_counts_when_timeouts_cannot_be_changed() {
    let (reply, ran, _) = serve_over(true, |stream| Box::new(ClosedPeerTimeouts(stream)));

    assert!(matches!(reply, DaemonReply::Ready { .. }), "{reply:?}");
    assert!(
        ran,
        "a readable go is never discarded over a refused timeout change"
    );
}
