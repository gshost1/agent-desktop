use super::serve_connection;
use crate::daemon::build_identity::BuildIdentity;
use crate::daemon::endpoint::DaemonEndpoint;
use crate::daemon::env_subset::EnvSubset;
use crate::daemon::reply::{DaemonReply, GO};
use crate::daemon::request::{DaemonRequest, PROTOCOL_VERSION};
use crate::daemon::server_state::ServerState;
use crate::daemon::wire::{REPLY_LIMIT, read_message, write_message};
use std::os::unix::net::UnixStream;
use std::sync::mpsc::channel;

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
        scope.spawn(|| serve_connection(Box::new(daemon_end), &state, &jobs));
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
