use super::binary;

const UNKNOWN_SKILL: [&str; 3] = ["skills", "get", "definitely-not-a-skill"];

fn run(args: &[&str], summary: Option<&str>) -> std::process::Output {
    let mut command = binary();
    command.args(args).env_remove("RUST_LOG");
    match summary {
        Some(value) => command.env("AGENT_DESKTOP_STDERR_SUMMARY", value),
        None => command.env_remove("AGENT_DESKTOP_STDERR_SUMMARY"),
    };
    command.output().expect("binary starts")
}

#[test]
fn an_error_envelope_also_prints_one_summary_line_on_stderr() {
    let output = run(&UNKNOWN_SKILL, None);
    let envelope: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout is one JSON envelope");
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(envelope["ok"], false);
    let code = envelope["error"]["code"].as_str().expect("error code");
    let message = envelope["error"]["message"].as_str().expect("message");
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(
        stderr.starts_with(&format!("agent-desktop: {code}: {message}")),
        "{stderr}"
    );
}

#[test]
fn a_parse_failure_prints_the_summary_line_and_keeps_exit_two() {
    let output = run(&["--definitely-not-a-real-flag"], None);
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");

    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr.starts_with("agent-desktop: INVALID_ARGS: "),
        "{stderr}"
    );
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
}

#[test]
fn a_successful_command_prints_nothing_on_stderr() {
    let output = run(&["version"], None);

    assert!(output.status.success());
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn turning_the_summary_off_leaves_stderr_empty_and_stdout_unchanged() {
    let with_line = run(&UNKNOWN_SKILL, None);
    let without_line = run(&UNKNOWN_SKILL, Some("0"));

    assert!(without_line.stderr.is_empty());
    assert_eq!(with_line.stdout, without_line.stdout);
    assert_eq!(with_line.status.code(), without_line.status.code());
}
