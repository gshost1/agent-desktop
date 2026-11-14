#![cfg(unix)]

use std::time::Duration;

#[path = "daemon_process_support.rs"]
mod support;

use support::{Home, assert_same, is_alive, is_permission_rerun, processes_using, seed_ref, wait_gone, wait_until};

const MATRIX: &[&[&str]] = &[
    &["version"],
    &["skills", "list"],
    &["skills", "get", "desktop"],
    &["status"],
    &["permissions"],
    &["list-apps"],
    &["list-windows"],
    &["list-displays"],
    &["snapshot", "--app", "NoSuchAppForDaemonTests", "-i"],
    &["find", "--app", "NoSuchAppForDaemonTests", "--role", "button"],
    &["click", "@e1"],
    &["click", "@sdaemonmissing:e1"],
    &["get", "@sdaemonmissing:e1", "--property", "text"],
    &["wait", "20"],
    &["session", "list"],
    &["batch", r#"[{"command":"version","args":{}},{"command":"wait","args":{"ms":5}}]"#],
    &["version", "--lines"],
];

const DECLINED: &[&[&str]] = &[
    &["snapshot", "--no-such-flag"],
    &["--help"],
    &["status", "--verbose"],
    &["launch", "NoSuchAppForDaemonTests"],
    &[],
];

#[test]
fn every_command_answers_the_same_bytes_and_status_in_both_modes() {
    let home = Home::new();
    let pid = home.warm();
    for args in MATRIX {
        let in_process = home.run(false, args);
        let before = home.served(pid);
        let through_daemon = home.run(true, args);
        assert_same(args, &in_process, &through_daemon);
        assert!(
            home.served(pid) == before + 1 || is_permission_rerun(&through_daemon),
            "{args:?} must be served by the daemon"
        );
    }
    for args in DECLINED {
        let in_process = home.run(false, args);
        let before = home.served(pid);
        let through_daemon = home.run(true, args);
        assert_same(args, &in_process, &through_daemon);
        assert_eq!(home.served(pid), before, "{args:?} runs in the calling process");
    }
    home.stop_daemons();
    assert_eq!(processes_using(&home.state_root()), Vec::<i32>::new());
}

#[test]
fn first_call_starts_one_private_daemon_and_runs_itself() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let home = Home::new();
    let first = home.run(true, &["version"]);
    assert!(first.status.success());
    let pid = wait_until(Duration::from_secs(10), || home.daemons().first().map(|d| d.0)).unwrap();
    assert_eq!(home.served(pid), 0, "the starting call ran in its own process");

    let dir = home.state_root().join("daemon");
    let mode = std::fs::metadata(&dir).unwrap().permissions().mode();
    let socket = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| entry.path().extension().is_some_and(|ext| ext == "sock"))
        .unwrap();
    let socket_mode = socket.metadata().unwrap().mode();
    assert_eq!(mode & 0o777, 0o700);
    assert_eq!(socket_mode & 0o777, 0o600);

    home.warm();
    assert_eq!(home.daemons().len(), 1, "one daemon per user, build, session and environment");
    home.stop_daemons();
}

#[test]
fn each_session_and_environment_gets_its_own_daemon() {
    let home = Home::new();
    home.warm();
    home.run(true, &["version", "--session", "daemon-a"]);
    let mut other_display = home.command(true, &["version"]);
    other_display.env("DISPLAY", ":199");
    other_display.output().unwrap();
    let three = wait_until(Duration::from_secs(10), || (home.daemons().len() == 3).then_some(()));
    home.stop_daemons();

    assert!(three.is_some(), "daemons: {:?}", home.daemons());
}

#[test]
fn daemon_restart_mid_session_keeps_refs_on_disk_and_qualified() {
    let home = Home::new();
    let stale = seed_ref(&home, "sdaemonrefs1", 999_999);
    let args = ["click", stale.as_str()];
    let in_process = home.run(false, &args);
    let pid = home.warm();
    assert_same(&args, &in_process, &home.run(true, &args));

    unsafe { libc::kill(pid, libc::SIGTERM) };
    assert!(wait_gone(pid));
    let after_restart = home.warm();
    assert_ne!(after_restart, pid);
    let served = home.served(after_restart);
    assert_same(&args, &in_process, &home.run(true, &args));
    assert_eq!(home.served(after_restart), served + 1);
    assert_same(&["click", "@e1"], &home.run(false, &["click", "@e1"]), &home.run(true, &["click", "@e1"]));
    home.stop_daemons();
}

#[test]
fn concurrent_clients_all_get_complete_answers() {
    let home = Home::new();
    let pid = home.warm();
    let before = home.served(pid);
    let clients: Vec<_> = (0..8).map(|_| home.spawn(true, &["wait", "300"])).collect();
    let outputs: Vec<_> = clients
        .into_iter()
        .map(|child| child.wait_with_output().unwrap())
        .collect();
    let reference = home.run(false, &["wait", "300"]);
    for output in &outputs {
        assert_same(&["wait", "300"], &reference, output);
    }
    let served = home.served(pid) - before;
    home.stop_daemons();

    assert!(served >= 1, "the daemon served at least one of the concurrent calls");
    assert!(served < 8, "busy calls ran in their own process instead of queueing");
}

#[test]
fn client_killed_mid_command_stops_its_daemon_like_a_killed_cli() {
    let home = Home::new();
    let pid = home.warm();
    let before = home.served(pid);
    let mut client = home.spawn(true, &["wait", "5000"]);
    std::thread::sleep(Duration::from_millis(500));
    client.kill().unwrap();
    client.wait().unwrap();

    assert!(wait_gone(pid), "the daemon stops with the client whose command it ran");
    assert_eq!(home.served(pid), before);
    assert!(home.run(true, &["version"]).status.success());
    let replacement = home.warm();
    assert_ne!(replacement, pid);
    home.stop_daemons();
    assert_eq!(processes_using(&home.state_root()), Vec::<i32>::new());
}

#[test]
fn command_past_its_timeout_answers_the_same_in_both_modes() {
    let home = Home::new();
    home.warm();
    let args = ["wait", "--window", "NoSuchWindowForDaemonTests", "--timeout", "300"];
    let in_process = home.run(false, &args);
    let through_daemon = home.run(true, &args);
    home.stop_daemons();

    assert_same(&args, &in_process, &through_daemon);
}

#[test]
fn daemon_killed_between_commands_is_replaced_without_a_failed_call() {
    let home = Home::new();
    let pid = home.warm();
    unsafe { libc::kill(pid, libc::SIGKILL) };
    assert!(wait_gone(pid));

    let args = ["get", "@sdaemonmissing:e1", "--property", "text"];
    assert_same(&args, &home.run(false, &args), &home.run(true, &args));
    let replacement = home.warm();
    assert_ne!(replacement, pid);
    home.stop_daemons();
}

#[test]
fn daemon_killed_during_a_read_only_command_lets_the_client_run_it() {
    let home = Home::new();
    let pid = home.warm();
    let client = home.spawn(true, &["wait", "1500"]);
    std::thread::sleep(Duration::from_millis(400));
    unsafe { libc::kill(pid, libc::SIGKILL) };
    let output = client.wait_with_output().unwrap();

    assert_same(&["wait", "1500"], &home.run(false, &["wait", "1500"]), &output);
    assert!(!is_alive(pid));
    home.stop_daemons();
}

#[test]
fn idle_daemon_exits_and_removes_its_files() {
    let home = Home::new();
    let mut first = home.command(true, &["version"]);
    first.env("AGENT_DESKTOP_DAEMON_IDLE_MS", "300");
    first.output().unwrap();
    let pid = wait_until(Duration::from_secs(10), || home.daemons().first().map(|d| d.0)).unwrap();

    assert!(wait_gone(pid), "an idle daemon exits");
    let left: Vec<_> = std::fs::read_dir(home.state_root().join("daemon"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name())
        .filter(|name| !name.to_string_lossy().ends_with(".lock"))
        .collect();
    assert!(left.is_empty(), "left behind: {left:?}");
}

#[test]
fn another_build_of_the_binary_gets_its_own_daemon() {
    let mut home = Home::new();
    home.warm();
    let copy = home.root.join("agent-desktop-copy");
    std::fs::copy(&home.binary, &copy).unwrap();
    let original = std::mem::replace(&mut home.binary, copy);
    home.run(true, &["version"]);
    let two = wait_until(Duration::from_secs(10), || (home.daemons().len() == 2).then_some(()));
    home.binary = original;
    home.stop_daemons();

    assert!(two.is_some(), "daemons: {:?}", home.daemons());
}

#[test]
fn daemon_mode_off_starts_no_daemon() {
    let home = Home::new();
    for _ in 0..3 {
        assert!(home.run(false, &["version"]).status.success());
    }

    assert!(!home.state_root().join("daemon").exists());
}
