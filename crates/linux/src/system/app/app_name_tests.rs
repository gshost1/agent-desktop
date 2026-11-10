use super::*;
use crate::system::process::process_facts::ProcessFacts;

fn facts(comm: &str, exe: Option<&str>) -> ProcessFacts {
    ProcessFacts {
        executable: None,
        comm: comm.to_owned(),
        exe_basename: exe.map(str::to_owned),
        instance: "boot:1".to_owned(),
    }
}

#[test]
fn the_name_is_the_executable_basename_whatever_the_root_would_say() {
    let process = facts("org.gnome.Nauti", Some("org.gnome.Nautilus"));
    assert_eq!(app_name(&process), "org.gnome.Nautilus");
}

#[test]
fn the_name_prefers_the_executable_over_the_cut_comm() {
    let process = facts("gnome-terminal-", Some("gnome-terminal-server"));
    assert_eq!(app_name(&process), "gnome-terminal-server");
}

#[test]
fn a_missing_or_blank_executable_falls_back_to_comm() {
    assert_eq!(app_name(&facts("python", None)), "python");
    assert_eq!(app_name(&facts("code", Some("  "))), "code");
}

#[test]
fn an_executable_deleted_while_it_runs_keeps_its_name() {
    let replaced = facts("calc", Some("gnome-calculator (deleted)"));
    assert_eq!(app_name(&replaced), "gnome-calculator");
    assert_eq!(app_name(&facts("x", Some(" (deleted)"))), "x");
}

fn aliased(query: &str, process: &ProcessFacts, aliases: &[&str]) -> bool {
    let aliases: Vec<String> = aliases.iter().map(|alias| (*alias).to_owned()).collect();
    name_matches(query, &app_name(process), &aliases)
}

#[test]
fn an_alias_matches_exactly_ignoring_case_and_never_renames_the_app() {
    let files = facts("nautilus", Some("nautilus"));
    assert!(aliased("Files", &files, &["Files"]));
    assert!(aliased("files", &files, &["Files"]));
    assert!(aliased("nautilus", &files, &["Files"]));
    assert!(!aliased("Files", &files, &[]));
    assert!(!aliased("File", &files, &["Files"]));
    assert!(!aliased("Files Helper", &files, &["Files"]));
    assert_eq!(app_name(&files), "nautilus");
}

fn launched_index() -> AliasIndex {
    AliasIndex::build(
        &[(
            "com.example.Browser".to_owned(),
            "[Desktop Entry]\nType=Application\nName=Example Browser\nExec=/opt/example/launcher %U\n"
                .to_owned(),
        )],
        &[],
    )
}

fn wrapped_process() -> ProcessFacts {
    ProcessFacts {
        executable: Some(
            crate::system::process::executable_identity::ExecutableIdentity {
                executable: std::path::PathBuf::from("/opt/example/browser-binary"),
                argv: Some(vec!["/opt/example/launcher".into(), "--flag".into()]),
            },
        ),
        ..facts("browser-binary", Some("browser-binary"))
    }
}

#[test]
fn a_process_launch_started_from_an_entry_answers_to_that_entry() {
    let index = launched_index();
    let process = wrapped_process();
    assert!(aliases_with(&index, &process, None).is_empty());
    let aliases = aliases_with(&index, &process, Some("com.example.Browser".to_owned()));
    assert_eq!(aliases, ["Example Browser", "com.example.Browser"]);
    assert_eq!(
        desktop_id_with(&index, &process, Some("com.example.Browser".to_owned())).as_deref(),
        Some("com.example.Browser")
    );
    assert_eq!(desktop_id_with(&index, &process, None), None);
}

#[test]
fn a_recorded_entry_that_no_longer_exists_names_nothing() {
    let index = launched_index();
    let process = wrapped_process();
    assert!(aliases_with(&index, &process, Some("gone.desktop.id".to_owned())).is_empty());
    assert_eq!(
        desktop_id_with(&index, &process, Some("gone.desktop.id".to_owned())),
        None
    );
}
