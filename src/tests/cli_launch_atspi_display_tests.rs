use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A display number nothing listens on or has left a socket for.
fn unused_display() -> String {
    (1900..2000)
        .find(|n| {
            !Path::new(&format!("/tmp/.X11-unix/X{n}")).exists()
                && !Path::new(&format!("/tmp/.X{n}-lock")).exists()
        })
        .map(|n| format!(":{n}"))
        .expect("a free display number")
}

/// Runs one command with an unreachable display that a session bus of its
/// own also inherits, inside the test's private session. The AT-SPI services that bus activates exit on the dead
/// display, so their calls get no reply.
fn run_on_dead_display(scratch: &Path, display: &str, args: &[&str]) -> serde_json::Value {
    let output = std::process::Command::new("dbus-run-session")
        .arg("--")
        .arg(env!("CARGO_BIN_EXE_agent-desktop"))
        .args(args)
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("AT_SPI_BUS_ADDRESS")
        .env("DISPLAY", display)
        .env("AGENT_DESKTOP_HOME", scratch.join("home"))
        .output()
        .expect("dbus-run-session starts");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .find(|line| line.starts_with("{\"version\""))
        .expect("stdout holds the JSON envelope");
    serde_json::from_str(line).expect("the envelope parses")
}

/// An accessibility bus that dies with the display is reported as the
/// display, never as an internal error or a missing permission: commands that
/// read the bus, and those that check permissions first, such as `snapshot`
/// and `launch`, answer the display refusal, and a launch spawns nothing.
#[test]
fn an_accessibility_bus_that_dies_with_the_display_is_reported_as_the_display() {
    let runs = std::process::Command::new("dbus-run-session")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success());
    let atspi = [
        "/usr/share/dbus-1/services/org.a11y.Bus.service",
        "/usr/local/share/dbus-1/services/org.a11y.Bus.service",
    ]
    .iter()
    .any(|service| Path::new(service).exists());
    if !runs || !atspi {
        eprintln!("skipped: needs dbus-run-session and an activatable AT-SPI bus (at-spi2-core)");
        return;
    }
    if super::support::run_in_private_session() {
        return;
    }
    let guard =
        Scratch(std::env::temp_dir().join(format!("adl-atspi-display-{}", std::process::id())));
    let scratch = guard.0.clone();
    std::fs::create_dir_all(&scratch).expect("scratch");
    let display = unused_display();
    let marker = scratch.join("started");
    let program = scratch.join("adl-atspi-display-program");
    std::fs::write(
        &program,
        format!("#!/bin/sh\ntouch '{}'\n", marker.display()),
    )
    .expect("a program");
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).expect("mode");

    let listed = run_on_dead_display(&scratch, &display, &["list-apps"]);
    let snapshot = run_on_dead_display(&scratch, &display, &["snapshot", "--app", "Absent"]);
    let status = run_on_dead_display(&scratch, &display, &["status"]);
    let launched = run_on_dead_display(
        &scratch,
        &display,
        &[
            "launch",
            program.to_str().expect("utf8"),
            "--timeout",
            "3000",
        ],
    );
    let started = (0..10).any(|_| {
        std::thread::sleep(std::time::Duration::from_millis(50));
        marker.exists()
    });
    drop(guard);

    for answer in [&listed, &snapshot, &launched] {
        assert_eq!(
            answer["error"]["code"], "PLATFORM_NOT_SUPPORTED",
            "{answer}"
        );
        assert!(
            answer["error"]["details"]["display_unavailable"]
                .as_str()
                .is_some_and(|cause| cause != "not_configured"),
            "{answer}"
        );
        assert!(
            answer["error"]["details"]["bus_error"].is_object(),
            "{answer}"
        );
    }
    assert_eq!(
        launched["error"]["disposition"]["delivery"], "not_delivered",
        "{launched}"
    );
    assert_eq!(
        status["data"]["permissions"]["accessibility"]["state"], "denied",
        "{status}"
    );
    assert!(!started, "the launch started the program");
}
