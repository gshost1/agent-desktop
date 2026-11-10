use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// Runs one command with an unreachable display that its private session
/// bus also inherits. The AT-SPI services that bus activates exit on the dead
/// display, so their calls get no reply.
fn on_dead_display(scratch: &Path, args: &[&str]) -> serde_json::Value {
    let output = std::process::Command::new("dbus-run-session")
        .arg("--")
        .arg(env!("CARGO_BIN_EXE_agent-desktop"))
        .args(args)
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("AT_SPI_BUS_ADDRESS")
        .env("DISPLAY", ":1987")
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
/// display, never as an internal error: commands that read the bus answer the
/// display refusal, and a launch, which checks permissions first, is refused
/// with the display named and nothing spawned.
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
    let scratch = std::env::temp_dir().join(format!("adl-atspi-display-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).expect("scratch");
    let marker = scratch.join("started");
    let program = scratch.join("adl-atspi-display-program");
    std::fs::write(
        &program,
        format!("#!/bin/sh\ntouch '{}'\n", marker.display()),
    )
    .expect("a program");
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).expect("mode");

    let listed = on_dead_display(&scratch, &["list-apps"]);
    let launched = on_dead_display(
        &scratch,
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
    let _ = std::fs::remove_dir_all(&scratch);

    assert_eq!(
        listed["error"]["code"], "PLATFORM_NOT_SUPPORTED",
        "{listed}"
    );
    assert_eq!(
        listed["error"]["details"]["display_unavailable"], "missing",
        "{listed}"
    );
    assert!(
        listed["error"]["details"]["bus_error"].is_object(),
        "{listed}"
    );
    assert_eq!(launched["error"]["code"], "PERM_DENIED", "{launched}");
    assert!(
        launched["error"]["suggestion"]
            .as_str()
            .is_some_and(|text| text.contains("DISPLAY")),
        "{launched}"
    );
    assert_eq!(
        launched["error"]["disposition"]["delivery"], "not_delivered",
        "{launched}"
    );
    assert!(!started, "the launch started the program");
}
