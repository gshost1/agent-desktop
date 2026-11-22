//! Windows has no daemon transport, so daemon mode fails closed: `AGENT_DESKTOP_DAEMON=1` and
//! `--daemon` run the command in the calling process with the same bytes and exit code, and
//! leave no daemon state under the state root.
#![cfg(target_os = "windows")]

use std::path::Path;
use std::process::{Command, Output};

#[path = "unique_scratch_dir.rs"]
mod unique_scratch_dir;
use unique_scratch_dir::unique_scratch_dir;

enum Mode {
    Off,
    Variable,
    Flag,
}

fn run(root: &Path, mode: &Mode, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_agent-desktop"));
    command
        .env("AGENT_DESKTOP_HOME", root)
        .env_remove("AGENT_DESKTOP_DAEMON")
        .env_remove("AGENT_DESKTOP_SESSION");
    match mode {
        Mode::Off => {}
        Mode::Variable => {
            command.env("AGENT_DESKTOP_DAEMON", "1");
        }
        Mode::Flag => {
            command.arg("--daemon");
        }
    }
    command.args(args).output().expect("the binary starts")
}

#[test]
fn daemon_mode_on_windows_runs_in_process_with_the_same_answer_and_no_daemon_state() {
    let root = unique_scratch_dir("daemon", "fallback");
    for args in [
        &["version"][..],
        &["skills", "list"],
        &["click", "@sdaemonmissing:e1"],
    ] {
        let plain = run(&root, &Mode::Off, args);
        for mode in [Mode::Variable, Mode::Flag] {
            let daemon = run(&root, &mode, args);
            assert_eq!(
                plain.status.code(),
                daemon.status.code(),
                "{args:?} exit code"
            );
            assert_eq!(plain.stdout, daemon.stdout, "{args:?} stdout");
            assert_eq!(plain.stderr, daemon.stderr, "{args:?} stderr");
        }
    }
    let daemon_dir = root.join("daemon").exists();
    let _ = std::fs::remove_dir_all(&root);

    assert!(
        !daemon_dir,
        "no daemon state on a platform without a daemon transport"
    );
}
