use super::*;
use std::ffi::OsString;

fn mcp_matches(words: impl IntoIterator<Item = impl Into<OsString> + Clone>) -> ArgMatches {
    let matches = crate::root_command()
        .try_get_matches_from(words)
        .unwrap_or_else(|error| panic!("the words must parse: {error}"));
    match matches.subcommand() {
        Some(("mcp", mcp)) => mcp.clone(),
        _ => panic!("the words must name mcp"),
    }
}

fn globals(words: &[&str]) -> Result<Vec<String>, AppError> {
    collect_forwarded_globals(&mcp_matches(words))
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
fn visual_debug_flags_are_refused_so_nothing_is_forwarded() {
    let message = refusal(&[
        "agent-desktop",
        "--debug",
        "--screenshot",
        "/tmp/x.html",
        "mcp",
    ]);
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
fn invalid_identity_values_are_refused_at_startup() {
    let session = refusal(&["agent-desktop", "mcp", "--session", "not a session"]);
    assert!(session.to_lowercase().contains("session"), "{session}");
    let agent = refusal(&["agent-desktop", "mcp", "--agent-id", "not a valid id"]);
    assert!(agent.contains("Agent id"), "{agent}");
}

#[cfg(unix)]
#[test]
fn a_non_utf8_value_is_refused_instead_of_forwarded_lossily() {
    use std::os::unix::ffi::OsStringExt;
    let path = OsString::from_vec(b"/tmp/trace-\xff.jsonl".to_vec());
    let words: [OsString; 4] = ["agent-desktop".into(), "mcp".into(), "--trace".into(), path];
    let Err(error) = collect_forwarded_globals(&mcp_matches(words)) else {
        panic!("a non-UTF-8 --trace must be refused");
    };
    assert_eq!(error.code(), "INVALID_ARGS");
    assert!(error.to_string().contains("--trace"), "{error}");
}

#[test]
fn server_wide_flags_are_forwarded_in_order_and_server_only_flags_are_not() {
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
            "--session=s1",
            "--agent-id=agent-a",
            "--trace=/tmp/t.jsonl",
            "--trace-strict",
            "--headed",
        ]
    );
    assert_eq!(
        globals(&["agent-desktop", "mcp", "-v"]).ok(),
        Some(Vec::new())
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

#[test]
fn an_unclassified_or_short_only_global_is_an_internal_error_not_a_silent_drop() {
    for (id, short) in [("mystery", 'm'), ("headed", 'H')] {
        let root = Command::new("agent-desktop")
            .arg(
                Arg::new(id)
                    .short(short)
                    .global(true)
                    .action(ArgAction::SetTrue),
            )
            .subcommand(Command::new("mcp"));
        let flag = format!("-{short}");
        let matches = root
            .clone()
            .try_get_matches_from(["agent-desktop", "mcp", flag.as_str()])
            .unwrap_or_else(|error| panic!("{error}"));
        let Some(("mcp", mcp)) = matches.subcommand() else {
            panic!("mcp");
        };
        let Err(error) = collect_from(&root, mcp) else {
            panic!("{flag} must be refused, not dropped");
        };
        assert_eq!(error.code(), "INTERNAL", "{flag}");
        let message = error.to_string();
        assert!(message.contains("not classified for mcp"), "{message}");
        assert!(message.contains(&flag), "{message}");
    }
}
