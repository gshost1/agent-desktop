use std::time::Duration;

use clap::Parser;

use super::preflight_deadline;

fn remaining(args: &[&str]) -> Duration {
    let cli = crate::Cli::try_parse_from(args).expect("parses");
    preflight_deadline(&cli.command.expect("a command"), None)
        .expect("a deadline builds")
        .remaining()
}

#[test]
fn a_command_with_a_shorter_timeout_bounds_its_permission_read_by_it() {
    for args in [
        &[
            "agent-desktop",
            "snapshot",
            "--app",
            "X",
            "--timeout-ms",
            "1000",
        ][..],
        &[
            "agent-desktop",
            "click",
            "@s1234abcd:e1",
            "--timeout-ms",
            "1000",
        ][..],
        &[
            "agent-desktop",
            "find",
            "--app",
            "X",
            "--role",
            "button",
            "--timeout-ms",
            "1000",
        ][..],
    ] {
        let left = remaining(args);
        assert!(left <= Duration::from_millis(1_000), "{args:?}: {left:?}");
    }
}

#[test]
fn a_snapshot_without_a_timeout_bounds_its_permission_read_by_the_snapshot_default() {
    let left = remaining(&["agent-desktop", "snapshot", "--app", "X"]);
    assert!(left <= Duration::from_millis(3_000), "{left:?}");
    assert!(left > Duration::from_millis(2_000), "{left:?}");
}

#[test]
fn a_longer_zero_or_absent_timeout_keeps_the_standard_permission_read() {
    for args in [
        &[
            "agent-desktop",
            "click",
            "@s1234abcd:e1",
            "--timeout-ms",
            "60000",
        ][..],
        &[
            "agent-desktop",
            "click",
            "@s1234abcd:e1",
            "--timeout-ms",
            "0",
        ][..],
        &["agent-desktop", "status"][..],
    ] {
        let left = remaining(args);
        assert!(left <= Duration::from_millis(5_000), "{args:?}: {left:?}");
        assert!(left > Duration::from_millis(4_000), "{args:?}: {left:?}");
    }
}
