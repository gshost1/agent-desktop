use std::process::{Child, Output};
use std::time::{Duration, Instant};

use super::support::{Home, assert_same, is_alive, processes_using, sockets_in, wait_gone};

/// Matches `READY_TIMEOUT` and `STALL_LIMIT` in `src/daemon/client.rs`.
const READY_TIMEOUT: Duration = Duration::from_secs(2);
const STALL_LIMIT: Duration = Duration::from_secs(5);
const LONG_INPUT: &str = r#"[{"command":"wait","args":{"ms":3000}}]"#;

fn envelope(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap()
}

fn signal(pid: i32, signal: i32) {
    assert_eq!(unsafe { libc::kill(pid, signal) }, 0);
}

fn assert_in_doubt(output: &Output) {
    let envelope = envelope(output);
    assert_eq!(output.status.code(), Some(1), "{envelope}");
    assert_eq!(envelope["command"], "batch", "{envelope}");
    assert_eq!(envelope["error"]["code"], "INTERNAL", "{envelope}");
    assert_eq!(
        envelope["error"]["disposition"]["delivery"],
        "delivery_uncertain"
    );
    assert_eq!(envelope["error"]["disposition"]["retry"], "unsafe");
}

/// The client's output once it exits, failing the test when it is still waiting at `limit`:
/// a client that waits on a hung daemon without bound would otherwise hang the suite.
fn finish_within(mut client: Child, limit: Duration) -> Output {
    let started = Instant::now();
    while client.try_wait().unwrap().is_none() {
        if started.elapsed() > limit {
            client.kill().unwrap();
            client.wait().unwrap();
            panic!("the client was still waiting on its daemon after {limit:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    client.wait_with_output().unwrap()
}

fn assert_clean(home: &Home) {
    home.stop_daemons();
    assert_eq!(processes_using(&home.state_root()), Vec::<i32>::new());
    assert_eq!(sockets_in(home), Vec::<String>::new());
}

#[test]
fn client_killed_mid_command_stops_its_daemon_like_a_killed_cli() {
    let home = Home::new();
    let pid = home.warm();
    let mut client = home.spawn(true, &["wait", "5000"]);
    std::thread::sleep(Duration::from_millis(500));
    client.kill().unwrap();
    client.wait().unwrap();

    assert!(
        wait_gone(pid),
        "the daemon stops with the client whose command it ran"
    );
    assert!(home.run(true, &["version"]).status.success());
    let replacement = home.warm();
    assert_ne!(replacement, pid);
    assert_clean(&home);
}

#[test]
fn command_past_its_timeout_answers_the_same_in_both_modes() {
    let home = Home::new();
    home.warm();
    let args = [
        "wait",
        "--window",
        "NoSuchWindowForDaemonTests",
        "--timeout",
        "300",
    ];
    let in_process = home.run(false, &args);
    let through_daemon = home.run(true, &args);

    assert_same(&args, &in_process, &through_daemon);
    assert_clean(&home);
}

#[test]
fn daemon_killed_between_commands_is_replaced_without_a_failed_call() {
    let home = Home::new();
    let pid = home.warm();
    signal(pid, libc::SIGKILL);
    assert!(wait_gone(pid));

    let args = ["get", "@sdaemonmissing:e1", "--property", "text"];
    assert_same(&args, &home.run(false, &args), &home.run(true, &args));
    let replacement = home.warm();
    assert_ne!(replacement, pid);
    assert_clean(&home);
}

#[test]
fn daemon_killed_during_a_read_only_command_lets_the_client_run_it() {
    let home = Home::new();
    let pid = home.warm();
    let client = home.spawn(true, &["wait", "1500"]);
    std::thread::sleep(Duration::from_millis(400));
    signal(pid, libc::SIGKILL);
    let output = client.wait_with_output().unwrap();

    assert_same(
        &["wait", "1500"],
        &home.run(false, &["wait", "1500"]),
        &output,
    );
    assert!(!is_alive(pid));
    assert_clean(&home);
}

#[test]
fn daemon_killed_during_an_input_command_reports_uncertain_delivery_and_never_reruns_it() {
    let home = Home::new();
    let pid = home.warm();
    let started = Instant::now();
    let client = home.spawn(true, &["batch", LONG_INPUT]);
    std::thread::sleep(Duration::from_millis(400));
    signal(pid, libc::SIGKILL);
    let output = client.wait_with_output().unwrap();

    assert_in_doubt(&output);
    assert!(
        started.elapsed() < Duration::from_millis(2500),
        "no second run"
    );
    assert_clean(&home);
}

#[test]
fn hung_idle_daemon_is_bypassed_after_the_ready_timeout() {
    let home = Home::new();
    let pid = home.warm();
    signal(pid, libc::SIGSTOP);
    let before = home.served(pid);
    let started = Instant::now();
    let through_daemon = finish_within(home.spawn(true, &["version"]), READY_TIMEOUT * 3);
    let waited = started.elapsed();
    signal(pid, libc::SIGCONT);

    assert_same(
        &["version"],
        &home.run(false, &["version"]),
        &through_daemon,
    );
    assert!(
        waited >= READY_TIMEOUT && waited < READY_TIMEOUT * 3,
        "{waited:?}"
    );
    assert_eq!(home.served(pid), before, "the stopped daemon never ran it");
    assert_clean(&home);
}

#[test]
fn daemon_hung_mid_read_only_command_is_detected_and_the_client_runs_it() {
    let home = Home::new();
    let pid = home.warm();
    let started = Instant::now();
    let client = home.spawn(true, &["wait", "800"]);
    std::thread::sleep(Duration::from_millis(300));
    signal(pid, libc::SIGSTOP);
    let output = finish_within(client, STALL_LIMIT * 3);
    let waited = started.elapsed();
    signal(pid, libc::SIGKILL);

    assert_same(
        &["wait", "800"],
        &home.run(false, &["wait", "800"]),
        &output,
    );
    assert!(waited >= STALL_LIMIT, "{waited:?}");
    assert_clean(&home);
}

#[test]
fn daemon_hung_mid_input_command_reports_uncertain_delivery() {
    let home = Home::new();
    let pid = home.warm();
    let started = Instant::now();
    let client = home.spawn(true, &["batch", LONG_INPUT]);
    std::thread::sleep(Duration::from_millis(400));
    signal(pid, libc::SIGSTOP);
    let output = finish_within(client, STALL_LIMIT * 3);
    let waited = started.elapsed();
    signal(pid, libc::SIGKILL);

    assert_in_doubt(&output);
    assert!(
        waited >= STALL_LIMIT && waited < STALL_LIMIT * 2,
        "{waited:?}"
    );
    assert_clean(&home);
}
