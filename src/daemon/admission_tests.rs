use super::{Admitted, admit};

fn argv(args: &[&str]) -> Vec<String> {
    std::iter::once("agent-desktop")
        .chain(args.iter().copied())
        .map(str::to_owned)
        .collect()
}

#[test]
fn observation_commands_are_admitted_and_safe_to_rerun() {
    assert_eq!(
        admit(&argv(&["snapshot", "--app", "Finder", "-i"])),
        Ok(Admitted {
            command: "snapshot",
            rerun_safe: true
        })
    );
    assert_eq!(admit(&argv(&["status"])).map(|a| a.rerun_safe), Ok(true));
}

#[test]
fn input_commands_are_admitted_but_never_rerun() {
    assert_eq!(
        admit(&argv(&["click", "@s1234abcd:e1"])),
        Ok(Admitted {
            command: "click",
            rerun_safe: false
        })
    );
    assert_eq!(
        admit(&argv(&["session", "start"])).map(|a| a.rerun_safe),
        Ok(false)
    );
}

#[test]
fn invocations_that_depend_on_the_calling_process_are_declined() {
    assert_eq!(admit(&argv(&["type", "@s1234abcd:e1", "--stdin"])), Err("stdin"));
    assert_eq!(admit(&argv(&["clipboard-set", "--stdin"])), Err("stdin"));
    assert_eq!(admit(&argv(&["launch", "Calculator"])), Err("launch"));
    assert_eq!(admit(&argv(&["status", "--verbose"])), Err("verbose"));
    assert_eq!(admit(&argv(&["mcp"])), Err("mcp"));
    assert_eq!(admit(&argv(&[])), Err("help"));
    assert_eq!(admit(&argv(&["--help"])), Err("arguments"));
    assert_eq!(admit(&argv(&["snapshot", "--no-such-flag"])), Err("arguments"));
}

#[test]
fn batch_that_launches_or_cannot_be_read_is_declined() {
    let launches = r#"[{"command":"status","args":{}},{"command":"launch","args":{"app":"X"}}]"#;
    let reads = r#"[{"command":"status","args":{}},{"command":"list-apps","args":{}}]"#;

    assert_eq!(admit(&argv(&["batch", launches])), Err("launch"));
    assert_eq!(admit(&argv(&["batch", "not json"])), Err("launch"));
    assert_eq!(admit(&argv(&["batch", r#"[{"args":{}}]"#])), Err("launch"));
    assert_eq!(admit(&argv(&["batch", reads])).map(|a| a.command), Ok("batch"));
}
