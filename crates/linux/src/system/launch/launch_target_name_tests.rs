use super::tests::{argv_in, fixture};
use super::*;

fn code_of(result: Result<Vec<String>, ErrorCode>) -> ErrorCode {
    result.expect_err("an error")
}

#[test]
fn a_name_whose_entries_run_different_commands_is_ambiguous_and_names_each_one() {
    let fixture = fixture(
        &[
            ("two", "Name=Files\nType=Application\nExec=b"),
            ("one", "Name=Files\nType=Application\nExec=a"),
        ],
        &["a", "b"],
    );
    let error = resolve("files", &[], &fixture.search, &fixture.index)
        .err()
        .expect("an ambiguity");
    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
    assert!(error.message.contains("one, two"), "{}", error.message);
    let candidates = &error.details.as_ref().expect("details")["candidates"];
    assert_eq!(candidates[0]["desktop_id"], "one");
    assert_eq!(candidates[1]["desktop_id"], "two");
    assert!(
        candidates[0]["argv"][0]
            .as_str()
            .is_some_and(|program| program.ends_with("/bin/a"))
    );
    let suggestion = error.suggestion.as_deref().expect("a suggestion");
    assert!(
        suggestion.contains("agent-desktop launch one"),
        "{suggestion}"
    );
    assert!(!suggestion.contains("snapshot"), "{suggestion}");
}

#[test]
fn a_name_whose_entries_run_the_same_command_launches_the_first_desktop_id() {
    let fixture = fixture(
        &[
            ("vendor-tool", "Name=Tool\nType=Application\nExec=tool %U"),
            (
                "org.vendor.Tool",
                "Name=Tool\nType=Application\nExec=tool %U",
            ),
        ],
        &["tool"],
    );
    let program = fixture.search.path_dirs[0].join("tool");
    assert_eq!(
        argv_in(&fixture, "Tool").unwrap(),
        [program.to_str().unwrap()]
    );
}

#[test]
fn entries_that_differ_only_in_working_directory_are_still_ambiguous() {
    let fixture = fixture(
        &[
            ("one", "Name=Tool\nType=Application\nExec=tool\nPath=/tmp"),
            ("two", "Name=Tool\nType=Application\nExec=tool"),
        ],
        &["tool"],
    );
    assert_eq!(
        code_of(argv_in(&fixture, "Tool")),
        ErrorCode::AmbiguousTarget
    );
}

#[test]
fn an_entry_that_cannot_start_does_not_make_a_name_ambiguous() {
    let fixture = fixture(
        &[
            (
                "one",
                "Name=Tool\nType=Application\nExec=tool\nTryExec=missing-tool",
            ),
            ("two", "Name=Tool\nType=Application\nExec=other"),
        ],
        &["tool", "other"],
    );
    let program = fixture.search.path_dirs[0].join("other");
    assert_eq!(
        argv_in(&fixture, "Tool").unwrap(),
        [program.to_str().unwrap()]
    );
}

#[test]
fn a_name_none_of_whose_entries_can_start_reports_why_the_first_cannot() {
    let fixture = fixture(
        &[
            (
                "one",
                "Name=Tool\nType=Application\nExec=tool\nTerminal=true",
            ),
            (
                "two",
                "Name=Tool\nType=Application\nExec=tool\nTerminal=true",
            ),
        ],
        &["tool"],
    );
    assert_eq!(
        code_of(argv_in(&fixture, "Tool")),
        ErrorCode::ActionNotSupported
    );
}
