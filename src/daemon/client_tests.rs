use super::exchange;
use crate::daemon::env_subset::EnvSubset;
use crate::daemon::reply::{DaemonReply, GO};
use crate::daemon::request::{DaemonRequest, PROTOCOL_VERSION};
use crate::daemon::wire::{REQUEST_LIMIT, read_message, write_frame, write_message};
use std::os::unix::net::UnixStream;
use std::process::ExitCode;

fn request() -> DaemonRequest {
    DaemonRequest {
        protocol: PROTOCOL_VERSION,
        build: "test".into(),
        argv: vec!["agent-desktop".into(), "version".into()],
        cwd: "/".into(),
        umask: 0o022,
        env: EnvSubset::from_pairs(Vec::new()),
    }
}

/// What a fake daemon does after it reads the request.
struct Script {
    before_go: Vec<DaemonReply>,
    after_go: Vec<DaemonReply>,
    frames: bool,
}

/// Runs `exchange` against a fake daemon that follows `script`, then hangs up. Returns the
/// client's result and whether the daemon received the go.
fn exchange_with(script: Script) -> (Option<ExitCode>, bool) {
    let (client, mut server) = UnixStream::pair().unwrap();
    let daemon = std::thread::spawn(move || {
        let received: DaemonRequest = read_message(&mut server, REQUEST_LIMIT).unwrap();
        assert_eq!(received, request());
        for reply in &script.before_go {
            write_message(&mut server, reply).unwrap();
        }
        let offered_ready = matches!(script.before_go.last(), Some(DaemonReply::Ready { .. }));
        let go = offered_ready
            && read_message::<String>(&mut server, REQUEST_LIMIT).is_ok_and(|word| word == GO);
        for reply in &script.after_go {
            write_message(&mut server, reply).unwrap();
        }
        if script.frames {
            write_frame(&mut server, b"").unwrap();
            write_frame(&mut server, b"").unwrap();
        }
        go
    });
    let result = exchange(Box::new(client), &request());
    (result, daemon.join().unwrap())
}

fn ready(rerun_safe: bool) -> DaemonReply {
    DaemonReply::Ready {
        command: "click".into(),
        rerun_safe,
    }
}

fn after_ready(rerun_safe: bool, after_go: Vec<DaemonReply>, frames: bool) -> Script {
    Script {
        before_go: vec![ready(rerun_safe)],
        after_go,
        frames,
    }
}

#[test]
fn declined_command_runs_in_this_process_without_a_go() {
    let declined = DaemonReply::Declined {
        reason: "busy".into(),
    };
    let script = Script {
        before_go: vec![declined],
        after_go: Vec::new(),
        frames: false,
    };

    assert_eq!(exchange_with(script), (None, false));
}

#[test]
fn daemon_that_hangs_up_before_ready_leaves_the_command_to_this_process() {
    let script = Script {
        before_go: Vec::new(),
        after_go: Vec::new(),
        frames: false,
    };

    assert_eq!(exchange_with(script), (None, false));
}

#[test]
fn finished_command_exits_with_the_daemon_status() {
    let script = after_ready(
        false,
        vec![DaemonReply::Started, DaemonReply::Done { exit: 2 }],
        true,
    );

    assert_eq!(exchange_with(script), (Some(ExitCode::from(2)), true));
}

#[test]
fn heartbeats_before_the_result_are_skipped() {
    let replies = vec![
        DaemonReply::Started,
        DaemonReply::Running,
        DaemonReply::Running,
        DaemonReply::Done { exit: 0 },
    ];

    assert_eq!(
        exchange_with(after_ready(false, replies, true)),
        (Some(ExitCode::SUCCESS), true)
    );
}

#[test]
fn permission_rerun_runs_in_this_process() {
    let rerun = DaemonReply::Rerun {
        reason: "permission".into(),
    };
    let script = after_ready(false, vec![DaemonReply::Started, rerun], false);

    assert_eq!(exchange_with(script), (None, true));
}

#[test]
fn input_command_whose_daemon_stopped_before_started_runs_here() {
    assert_eq!(
        exchange_with(after_ready(false, Vec::new(), false)),
        (None, true)
    );
}

#[test]
fn read_only_command_whose_daemon_died_runs_again_here() {
    let script = after_ready(true, vec![DaemonReply::Started], false);

    assert_eq!(exchange_with(script), (None, true));
}

#[test]
fn input_command_whose_daemon_died_after_starting_is_never_run_again() {
    let script = after_ready(false, vec![DaemonReply::Started], false);

    assert_eq!(exchange_with(script), (Some(ExitCode::FAILURE), true));
}

#[test]
fn a_heartbeat_alone_proves_the_command_started() {
    let script = after_ready(false, vec![DaemonReply::Running], false);

    assert_eq!(exchange_with(script), (Some(ExitCode::FAILURE), true));
}

#[test]
fn truncated_output_of_an_input_command_is_reported_not_rerun() {
    let script = after_ready(
        false,
        vec![DaemonReply::Started, DaemonReply::Done { exit: 0 }],
        false,
    );

    assert_eq!(exchange_with(script), (Some(ExitCode::FAILURE), true));
}

#[test]
fn a_daemon_that_never_answers_is_left_after_the_ready_timeout_without_a_go() {
    let (client, mut server) = UnixStream::pair().unwrap();
    let daemon = std::thread::spawn(move || {
        let _: DaemonRequest = read_message(&mut server, REQUEST_LIMIT).unwrap();
        std::thread::sleep(super::READY_TIMEOUT * 2);
        server.set_nonblocking(true).unwrap();
        read_message::<String>(&mut server, REQUEST_LIMIT).is_ok()
    });
    let started = std::time::Instant::now();
    let result = exchange(Box::new(client), &request());
    let waited = started.elapsed();
    let got_go = daemon.join().unwrap();

    assert_eq!(result, None);
    assert!(
        !got_go,
        "a client that gave up never commits the daemon to run"
    );
    assert!(
        waited >= super::READY_TIMEOUT && waited < super::READY_TIMEOUT * 3 / 2,
        "the client gives up at its timeout, not when the daemon hangs up: {waited:?}"
    );
}
