use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

fn marking_program(scratch: &Path) -> (PathBuf, PathBuf) {
    let marker = scratch.join("started");
    let program = scratch.join("adl-no-display-program");
    let sleeper = scratch.join("adl-no-display-sleep");
    std::fs::copy("/bin/sleep", &sleeper).expect("copy sleep");
    std::fs::write(
        &program,
        format!(
            "#!/bin/sh\ntouch '{}'\nexec '{}' 30\n",
            marker.display(),
            sleeper.display()
        ),
    )
    .expect("a program");
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).expect("mode");
    (program, marker)
}

fn launch_with_display(display: Option<&str>, label: &str) -> (serde_json::Value, bool) {
    let scratch =
        std::env::temp_dir().join(format!("adl-no-display-{label}-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).expect("scratch");
    let (program, marker) = marking_program(&scratch);
    let mut command = super::binary();
    command
        .args([
            "launch",
            program.to_str().expect("utf8"),
            "--timeout",
            "3000",
        ])
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env("AGENT_DESKTOP_HOME", scratch.join("home"));
    if let Some(display) = display {
        command.env("DISPLAY", display);
    }
    let output = command.output().expect("binary starts");
    let started = (0..10).any(|_| {
        std::thread::sleep(std::time::Duration::from_millis(50));
        marker.exists()
    });
    let _ = std::process::Command::new("pkill")
        .args(["-f", &scratch.display().to_string()])
        .status();
    let _ = std::fs::remove_dir_all(&scratch);
    let envelope = serde_json::from_slice(&output.stdout).expect("stdout is one JSON envelope");
    (envelope, started)
}

#[test]
fn a_launch_without_a_display_server_starts_nothing() {
    for (display, label, cause) in [
        (None, "unset", "not_configured"),
        (Some(":1987"), "missing", "missing"),
    ] {
        let (envelope, started) = launch_with_display(display, label);
        assert_eq!(envelope["ok"], false, "{label}: {envelope}");
        assert_eq!(
            envelope["error"]["code"], "PLATFORM_NOT_SUPPORTED",
            "{envelope}"
        );
        assert_eq!(
            envelope["error"]["disposition"]["delivery"], "not_delivered",
            "{envelope}"
        );
        assert_eq!(
            envelope["error"]["details"]["display_unavailable"], cause,
            "{envelope}"
        );
        assert!(!started, "{label}: the launch started the program");
    }
}
