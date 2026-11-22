//! The CLI process contract suite with daemon mode on: every case of `cli_process.rs` runs again
//! with `AGENT_DESKTOP_DAEMON=1` and must hold unchanged, so the envelope, exit codes and stderr
//! contract is the same through the daemon as in-process.
#![cfg(unix)]

#[path = "cli_process.rs"]
mod suite;

#[test]
fn suite_calls_reach_a_daemon() {
    let home = std::env::temp_dir().join(format!("cli-daemon-reach-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir(&home).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_agent-desktop"))
        .arg("version")
        .env("AGENT_DESKTOP_HOME", &home)
        .env("AGENT_DESKTOP_DAEMON", "1")
        .env("AGENT_DESKTOP_DAEMON_IDLE_MS", "3000")
        .output()
        .unwrap();
    let states: Vec<serde_json::Value> = std::fs::read_dir(home.join("daemon"))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "state"))
        .filter_map(|entry| serde_json::from_slice(&std::fs::read(entry.path()).ok()?).ok())
        .collect();
    for state in &states {
        if let Some(pid) = state["pid"]
            .as_i64()
            .and_then(|pid| i32::try_from(pid).ok())
        {
            unsafe { libc::kill(pid, libc::SIGTERM) };
        }
    }
    let _ = std::fs::remove_dir_all(&home);

    assert!(output.status.success());
    assert_eq!(states.len(), 1, "{states:?}");
    assert_eq!(states[0]["served"], 1, "the call ran in the daemon");
}
