use super::*;

fn globals(words: &[&str]) -> Result<Vec<String>, AppError> {
    let matches = crate::root_command()
        .try_get_matches_from(words)
        .unwrap_or_else(|error| panic!("{words:?} must parse: {error}"));
    let Some(("mcp", mcp)) = matches.subcommand() else {
        panic!("{words:?} must name mcp");
    };
    forwarded_globals(mcp)
}

fn refusal(words: &[&str]) -> String {
    match globals(words) {
        Ok(forwarded) => panic!("{words:?} must be refused, forwarded {forwarded:?}"),
        Err(error) => {
            assert_eq!(error.code(), "INVALID_ARGS", "{words:?}");
            assert!(error.suggestion().is_some(), "{words:?}");
            error.to_string()
        }
    }
}

#[test]
fn visual_debug_flags_are_refused_and_never_forwarded() {
    let words = [
        "agent-desktop",
        "--debug",
        "--screenshot",
        "/tmp/x.html",
        "mcp",
    ];
    let forwarded = globals(&words).unwrap_or_default();
    assert!(
        !forwarded
            .iter()
            .any(|flag| flag.starts_with("--debug") || flag.starts_with("--screenshot")),
        "{forwarded:?}"
    );
    let message = refusal(&words);
    assert!(message.contains("--debug"), "{message}");
    assert!(message.contains("--screenshot"), "{message}");
}

#[test]
fn post_action_wait_flags_are_refused_before_or_after_mcp() {
    for words in [
        &["agent-desktop", "--wait-for", "button:OK", "mcp"][..],
        &["agent-desktop", "mcp", "--wait-for-gone", "button:OK"],
        &[
            "agent-desktop",
            "mcp",
            "--wait-for",
            "button:OK",
            "--wait-timeout",
            "5",
        ],
        &["agent-desktop", "mcp", "--wait-timeout", "5"],
    ] {
        let message = refusal(words);
        assert!(message.contains("--wait-"), "{message}");
    }
}

#[test]
fn server_wide_flags_are_forwarded_in_order() {
    let forwarded = globals(&[
        "agent-desktop",
        "-v",
        "--session",
        "s1",
        "mcp",
        "--agent-id",
        "agent-a",
        "--trace",
        "/tmp/t.jsonl",
        "--trace-strict",
        "--headed",
    ])
    .unwrap_or_else(|error| panic!("server-wide flags must be forwarded: {error}"));
    assert_eq!(
        forwarded,
        [
            "--verbose",
            "--session=s1",
            "--agent-id=agent-a",
            "--trace=/tmp/t.jsonl",
            "--trace-strict",
            "--headed",
        ]
    );
    assert_eq!(globals(&["agent-desktop", "mcp"]).ok(), Some(Vec::new()));
}

#[test]
fn every_global_flag_has_a_scope() {
    for arg in Cli::command()
        .get_arguments()
        .filter(|arg| arg.is_global_set())
    {
        assert!(
            GlobalScope::of(arg.get_id().as_str()).is_some(),
            "classify --{:?} in GlobalScope::of",
            arg.get_long()
        );
    }
}
