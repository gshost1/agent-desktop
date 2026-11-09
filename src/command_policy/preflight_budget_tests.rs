use std::time::Duration;

use clap::Parser;

use super::preflight_deadline;

fn remaining(args: &[&str]) -> Duration {
    let cli = crate::Cli::try_parse_from(args).expect("parses");
    preflight_deadline(&cli.command.expect("a command"), None)
        .expect("a deadline builds")
        .remaining()
}

fn assert_bounded(args: &[&str], at_most_ms: u64) {
    let left = remaining(args);
    assert!(
        left <= Duration::from_millis(at_most_ms),
        "{args:?}: {left:?}"
    );
    assert!(
        left > Duration::from_millis(at_most_ms / 2),
        "{args:?}: {left:?}"
    );
}

#[test]
fn a_short_timeout_bounds_the_permission_read_by_it_plus_the_slack() {
    assert_bounded(
        &[
            "agent-desktop",
            "snapshot",
            "--app",
            "X",
            "--timeout-ms",
            "1000",
        ],
        1_900,
    );
    assert_bounded(
        &[
            "agent-desktop",
            "click",
            "@s1234abcd:e1",
            "--timeout-ms",
            "1000",
        ],
        1_900,
    );
    assert_bounded(
        &[
            "agent-desktop",
            "wait",
            "--window",
            "W",
            "--timeout",
            "1000",
        ],
        1_900,
    );
}

#[test]
fn a_tiny_or_zero_timeout_still_leaves_a_healthy_permission_read_the_slack() {
    assert_bounded(
        &[
            "agent-desktop",
            "click",
            "@s1234abcd:e1",
            "--timeout-ms",
            "0",
        ],
        900,
    );
    assert_bounded(
        &[
            "agent-desktop",
            "click",
            "@s1234abcd:e1",
            "--timeout-ms",
            "50",
        ],
        950,
    );
}

#[test]
fn a_snapshot_without_a_timeout_uses_the_snapshot_default() {
    assert_bounded(&["agent-desktop", "snapshot", "--app", "X"], 3_900);
}

#[test]
fn a_long_or_absent_timeout_keeps_the_standard_permission_read() {
    assert_bounded(
        &[
            "agent-desktop",
            "click",
            "@s1234abcd:e1",
            "--timeout-ms",
            "60000",
        ],
        5_000,
    );
    assert_bounded(&["agent-desktop", "status"], 5_000);
}
