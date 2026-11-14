use super::exchange;
use crate::daemon::env_subset::EnvSubset;
use crate::daemon::reply::DaemonReply;
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

/// Runs `exchange` against a fake daemon that reads the request, sends `replies` and then
/// either sends the output frames (when `frames`) or hangs up.
fn exchange_with(replies: Vec<DaemonReply>, frames: bool) -> Option<ExitCode> {
    let (client, mut server) = UnixStream::pair().unwrap();
    let daemon = std::thread::spawn(move || {
        let received: DaemonRequest = read_message(&mut server, REQUEST_LIMIT).unwrap();
        assert_eq!(received, request());
        for reply in replies {
            write_message(&mut server, &reply).unwrap();
        }
        if frames {
            write_frame(&mut server, b"").unwrap();
            write_frame(&mut server, b"").unwrap();
        }
    });
    let result = exchange(client, &request());
    daemon.join().unwrap();
    result
}

fn accepted(rerun_safe: bool) -> DaemonReply {
    DaemonReply::Accepted {
        command: "click".into(),
        rerun_safe,
    }
}

#[test]
fn declined_command_runs_in_this_process() {
    let declined = DaemonReply::Declined {
        reason: "busy".into(),
    };

    assert_eq!(exchange_with(vec![declined], false), None);
}

#[test]
fn daemon_that_hangs_up_before_accepting_leaves_the_command_to_this_process() {
    assert_eq!(exchange_with(Vec::new(), false), None);
}

#[test]
fn finished_command_exits_with_the_daemon_status() {
    let done = DaemonReply::Done { exit: 2 };

    assert_eq!(
        exchange_with(vec![accepted(false), done], true),
        Some(ExitCode::from(2))
    );
}

#[test]
fn permission_rerun_runs_in_this_process() {
    let rerun = DaemonReply::Rerun {
        reason: "permission".into(),
    };

    assert_eq!(exchange_with(vec![accepted(false), rerun], false), None);
}

#[test]
fn read_only_command_whose_daemon_died_runs_again_here() {
    assert_eq!(exchange_with(vec![accepted(true)], false), None);
}

#[test]
fn input_command_whose_daemon_died_is_never_run_again() {
    assert_eq!(
        exchange_with(vec![accepted(false)], false),
        Some(ExitCode::FAILURE)
    );
}

#[test]
fn truncated_output_of_an_input_command_is_reported_not_rerun() {
    let done = DaemonReply::Done { exit: 0 };

    assert_eq!(
        exchange_with(vec![accepted(false), done], false),
        Some(ExitCode::FAILURE)
    );
}
