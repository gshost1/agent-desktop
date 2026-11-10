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
            "#!/bin/sh\necho $$ > '{marker}.tmp' && mv '{marker}.tmp' '{marker}'\nexec '{}' 30\n",
            sleeper.display(),
            marker = marker.display()
        ),
    )
    .expect("a program");
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).expect("mode");
    (program, marker)
}

/// Stops the one process the fixture recorded: the pid it wrote, which the
/// `exec` keeps, only while that pid still runs the fixture's own sleeper.
fn stop_recorded(marker: &Path, sleeper: &Path) {
    let Some(pid) = std::fs::read_to_string(marker)
        .ok()
        .and_then(|text| text.trim().parse::<u32>().ok())
    else {
        return;
    };
    let runs_sleeper = std::fs::read(format!("/proc/{pid}/cmdline")).is_ok_and(|cmdline| {
        cmdline
            .split(|byte| *byte == 0)
            .next()
            .is_some_and(|program| program == sleeper.as_os_str().as_encoded_bytes())
    });
    if runs_sleeper {
        let _ = std::process::Command::new("kill")
            .arg(pid.to_string())
            .status();
    }
}

/// The binary on a session bus of its own, so its launch gets past the bus
/// preflight to the display check without reaching any real session.
/// `dbus-run-session` tears that bus down when the binary exits. The bus and
/// the accessibility services it activates start with no display at all,
/// because a launcher pointed at a dead display exits; only the binary gets
/// `display`. Those services share the command's stdout, so the envelope is
/// read as the line that starts one. Hosts without `dbus-run-session` run the
/// binary on the session they provide.
fn on_private_bus(display: Option<&str>) -> std::process::Command {
    let runner = Path::new("/usr/bin/dbus-run-session");
    let binary = env!("CARGO_BIN_EXE_agent-desktop");
    let mut command = if runner.exists() {
        let mut command = std::process::Command::new(runner);
        command.args(["--", "env"]);
        if let Some(display) = display {
            command.arg(format!("DISPLAY={display}"));
        }
        command.arg(binary);
        command
    } else {
        super::binary()
    };
    command.env_remove("DISPLAY").env_remove("WAYLAND_DISPLAY");
    if !runner.exists()
        && let Some(display) = display
    {
        command.env("DISPLAY", display);
    }
    command
}

fn launch_with_display(display: Option<&str>, label: &str) -> (serde_json::Value, bool) {
    let scratch =
        std::env::temp_dir().join(format!("adl-no-display-{label}-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).expect("scratch");
    let (program, marker) = marking_program(&scratch);
    let mut command = on_private_bus(display);
    command
        .args([
            "launch",
            program.to_str().expect("utf8"),
            "--timeout",
            "3000",
        ])
        .env("AGENT_DESKTOP_HOME", scratch.join("home"));
    let output = command.output().expect("binary starts");
    let started = (0..10).any(|_| {
        std::thread::sleep(std::time::Duration::from_millis(50));
        marker.exists()
    });
    stop_recorded(&marker, &scratch.join("adl-no-display-sleep"));
    let _ = std::fs::remove_dir_all(&scratch);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .find(|line| line.starts_with("{\"version\""))
        .expect("stdout holds the JSON envelope");
    let envelope = serde_json::from_str(line).expect("the envelope parses");
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
