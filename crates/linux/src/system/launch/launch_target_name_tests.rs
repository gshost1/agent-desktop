use super::tests::{argv_in, code_of, fixture};
use super::*;

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
fn an_entry_that_cannot_start_and_runs_another_command_keeps_the_name_ambiguous() {
    let fixture = fixture(
        &[
            (
                "one",
                "Name=Tool\nType=Application\nExec=tool\nTerminal=true",
            ),
            ("two", "Name=Tool\nType=Application\nExec=other"),
        ],
        &["tool", "other"],
    );
    let error = resolve("Tool", &[], &fixture.search, &fixture.index)
        .err()
        .expect("an ambiguity");
    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
    let candidates = &error.details.as_ref().expect("details")["candidates"];
    assert!(
        candidates[0]["refusal"]
            .as_str()
            .is_some_and(|why| why.contains("terminal"))
    );
    assert!(candidates[1]["argv"].is_array());
    let suggestion = error.suggestion.as_deref().expect("a suggestion");
    assert!(
        suggestion.contains("agent-desktop launch two"),
        "{suggestion}"
    );
}

#[test]
fn a_twin_that_cannot_start_does_not_block_the_entry_that_can() {
    let fixture = fixture(
        &[
            (
                "one",
                "Name=Tool\nType=Application\nExec=tool\nTryExec=missing-tool",
            ),
            ("two", "Name=Tool\nType=Application\nExec=tool"),
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
fn a_name_none_of_whose_entries_can_start_reports_why_the_first_cannot() {
    let fixture = fixture(
        &[
            (
                "two",
                "Name=Tool\nType=Application\nExec=tool\nTryExec=missing-tool",
            ),
            (
                "one",
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

#[test]
fn one_program_spelled_by_path_and_by_name_is_the_same_command() {
    let fixture = fixture(
        &[
            ("one", "Name=Tool\nType=Application\nExec=tool %U"),
            ("two", "Name=Tool\nType=Application\nExec=TOOLPATH %U"),
        ],
        &["tool"],
    );
    let program = fixture.search.path_dirs[0].join("tool");
    let rewritten = fixture.search.data_dirs[0].join("two.desktop");
    let text = std::fs::read_to_string(&rewritten)
        .unwrap()
        .replace("TOOLPATH", program.to_str().unwrap());
    std::fs::write(&rewritten, text).unwrap();
    let index = crate::system::app::alias_cache::AliasCache::new().index(&fixture.search.data_dirs);
    let plan = resolve("Tool", &[], &fixture.search, &index).expect("one command");
    assert_eq!(plan.argv, [program.to_str().unwrap()]);
}

#[test]
fn a_multi_call_binary_reached_under_another_name_is_another_command() {
    let fixture = fixture(
        &[
            ("one", "Name=Tool\nType=Application\nExec=tool %U"),
            ("two", "Name=Tool\nType=Application\nExec=tool-view %U"),
        ],
        &["tool"],
    );
    let program = fixture.search.path_dirs[0].join("tool");
    std::os::unix::fs::symlink(&program, fixture.search.path_dirs[0].join("tool-view")).unwrap();
    let index = crate::system::app::alias_cache::AliasCache::new().index(&fixture.search.data_dirs);
    let error = resolve("Tool", &[], &fixture.search, &index)
        .err()
        .expect("two commands");
    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
}

#[test]
fn a_desktop_file_shadowed_by_an_earlier_data_dir_records_no_desktop_id() {
    let fixture = fixture(
        &[("tool", "Name=Tool\nType=Application\nExec=tool")],
        &["tool"],
    );
    let shadowed_dir = fixture.search.data_dirs[0].with_file_name("shadowed-apps");
    std::fs::create_dir_all(&shadowed_dir).unwrap();
    let shadowed = shadowed_dir.join("tool.desktop");
    std::fs::write(
        &shadowed,
        "[Desktop Entry]\nName=Tool Old\nType=Application\nExec=tool\n",
    )
    .unwrap();
    let search = Search {
        data_dirs: vec![fixture.search.data_dirs[0].clone(), shadowed_dir.clone()],
        path_dirs: fixture.search.path_dirs.clone(),
    };
    let shadowed_plan = resolve(shadowed.to_str().unwrap(), &[], &search, &fixture.index)
        .expect("a trusted desktop file");
    assert_eq!(shadowed_plan.desktop_id, None);
    let indexed = search.data_dirs[0].join("tool.desktop");
    let indexed_plan = resolve(indexed.to_str().unwrap(), &[], &search, &fixture.index)
        .expect("the indexed desktop file");
    assert_eq!(indexed_plan.desktop_id.as_deref(), Some("tool"));
    let _ = std::fs::remove_dir_all(&shadowed_dir);
}

#[test]
fn a_desktop_file_reached_through_a_symlinked_data_dir_keeps_its_desktop_id() {
    let fixture = fixture(
        &[("tool", "Name=Tool\nType=Application\nExec=tool")],
        &["tool"],
    );
    let real_dir = fixture.search.data_dirs[0].clone();
    let linked_dir = real_dir.with_file_name("linked-apps");
    let _ = std::fs::remove_file(&linked_dir);
    std::os::unix::fs::symlink(&real_dir, &linked_dir).unwrap();
    let search = Search {
        data_dirs: vec![linked_dir.clone(), real_dir.clone()],
        path_dirs: fixture.search.path_dirs.clone(),
    };
    let file = real_dir.join("tool.desktop");
    let plan = resolve(file.to_str().unwrap(), &[], &search, &fixture.index)
        .expect("a trusted desktop file");
    assert_eq!(plan.desktop_id.as_deref(), Some("tool"));
    let _ = std::fs::remove_file(&linked_dir);
}
