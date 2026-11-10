use std::path::Path;

fn processes_running(program: &Path) -> Vec<u32> {
    let wanted = program.to_string_lossy().into_owned();
    std::fs::read_dir("/proc")
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok())
        .filter(|pid| {
            std::fs::read(format!("/proc/{pid}/cmdline")).is_ok_and(|bytes| {
                bytes
                    .split(|byte| *byte == 0)
                    .next()
                    .is_some_and(|first| first == wanted.as_bytes())
            })
        })
        .collect()
}

#[test]
fn a_launch_without_a_display_server_starts_nothing() {
    let scratch = std::env::temp_dir().join(format!("adl-no-display-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).expect("scratch");
    let program = scratch.join("adl-no-display-sleep");
    std::fs::copy("/bin/sleep", &program).expect("copy sleep");
    let output = super::binary()
        .args([
            "launch",
            program.to_str().expect("utf8"),
            "--arg",
            "30",
            "--timeout",
            "3000",
        ])
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env("AGENT_DESKTOP_HOME", scratch.join("home"))
        .output()
        .expect("binary starts");
    let started = processes_running(&program);
    for pid in &started {
        let _ = std::process::Command::new("kill")
            .arg(pid.to_string())
            .status();
    }
    let _ = std::fs::remove_dir_all(&scratch);
    let envelope: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout is one JSON envelope");
    assert_eq!(envelope["ok"], false, "{envelope}");
    assert_eq!(
        envelope["error"]["code"], "PLATFORM_NOT_SUPPORTED",
        "{envelope}"
    );
    assert_eq!(
        envelope["error"]["disposition"]["delivery"], "not_delivered",
        "{envelope}"
    );
    assert!(started.is_empty(), "the launch started {started:?}");
}
