use crate::cli::Cli;
use agent_desktop_core::commands::batch::parse_commands;
use clap::{CommandFactory, Parser};

const SKILL_DESKTOP: &str = include_str!("../../skills/agent-desktop/SKILL.md");
const SKILL_DESKTOP_DOCS: &[(&str, &str)] = &[
    ("SKILL.md", SKILL_DESKTOP),
    (
        "references/workflows.md",
        include_str!("../../skills/agent-desktop/references/workflows.md"),
    ),
    (
        "references/commands-system.md",
        include_str!("../../skills/agent-desktop/references/commands-system.md"),
    ),
    (
        "references/commands-interaction.md",
        include_str!("../../skills/agent-desktop/references/commands-interaction.md"),
    ),
];
const SKILL_MACOS: &str = include_str!("../../skills/agent-desktop-macos/SKILL.md");
const BATCH_OPENER: &str = "agent-desktop batch '";

fn batch_examples(text: &str) -> Vec<&str> {
    text.match_indices(BATCH_OPENER)
        .filter_map(|(at, _)| {
            let rest = &text[at + BATCH_OPENER.len()..];
            rest.split_once("]'")
                .map(|(body, _)| &rest[..body.len() + 1])
        })
        .collect()
}

fn split_command_line(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut started = false;
    for character in line.chars() {
        match character {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            ' ' if !quoted => {
                if started {
                    words.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            other => {
                current.push(other);
                started = true;
            }
        }
    }
    if started {
        words.push(current);
    }
    words
}

fn synopsis_lines() -> Vec<(String, String)> {
    let section = SKILL_DESKTOP
        .split_once("## Command synopsis")
        .expect("the core skill has a command synopsis")
        .1;
    let block = section
        .split("```")
        .nth(1)
        .expect("the synopsis is a fenced block");
    block
        .lines()
        .filter(|line| line.starts_with("agent-desktop "))
        .map(|line| {
            let (example, form) = line
                .split_once(" # ")
                .expect("each synopsis line carries its full form after ' # '");
            (example.trim().to_owned(), form.trim().to_owned())
        })
        .collect()
}

#[test]
fn every_batch_example_in_the_desktop_skill_parses() {
    let mut checked = 0;
    for (path, text) in SKILL_DESKTOP_DOCS {
        for example in batch_examples(text) {
            let entries = parse_commands(example)
                .unwrap_or_else(|error| panic!("{path}: batch example is not JSON: {error}"));
            assert!(!entries.is_empty(), "{path}: empty batch example");
            for entry in entries {
                let command = entry.command.clone();
                crate::batch::parse_command(entry).unwrap_or_else(|error| {
                    panic!("{path}: batch entry '{command}' does not decode: {error}")
                });
            }
            checked += 1;
        }
    }
    assert!(
        checked >= 3,
        "expected the shipped batch examples, found {checked}"
    );
}

#[test]
fn the_worked_batch_chain_takes_every_ref_from_before_the_batch() {
    let workflows = SKILL_DESKTOP_DOCS
        .iter()
        .find(|(path, _)| *path == "references/workflows.md")
        .expect("workflows reference")
        .1;
    let chain = batch_examples(workflows)
        .into_iter()
        .find(|example| example.contains("set-value") && example.contains("press"))
        .expect("the workflows reference carries the worked chain");
    let entries = parse_commands(chain).unwrap();
    assert!(
        entries.len() >= 4,
        "a chain that saves calls has several entries"
    );
    for entry in &entries {
        if let Some(reference) = entry.args.get("ref_id").and_then(|value| value.as_str()) {
            assert!(reference.starts_with("@s8f3k2p9:e"), "{reference}");
        }
    }
    assert!(
        entries
            .iter()
            .all(|entry| entry.command != "snapshot" && entry.command != "find"),
        "the chain must not rely on refs produced inside the batch"
    );
}

#[test]
fn the_macos_in_place_edit_batch_entries_decode() {
    let entries = parse_commands(
        r#"[{"command":"set-value","args":{"ref_id":"@s8f3k2p9:e1","value":"Reports"}},
            {"command":"press","args":{"combo":"return","app":"Finder"}}]"#,
    )
    .unwrap();
    for entry in entries {
        crate::batch::parse_command(entry).expect("in-place edit entry decodes");
    }
    assert!(SKILL_MACOS.contains(r#"{"combo":"return","app":"<App>"}"#));
}

#[test]
fn every_synopsis_example_parses_as_the_cli_would() {
    let lines = synopsis_lines();
    let names: Vec<&str> = lines
        .iter()
        .map(|(example, _)| example.split(' ').nth(1).unwrap())
        .collect();
    assert_eq!(names, ["press", "find", "type", "select", "set-value"]);
    for (example, _) in &lines {
        Cli::try_parse_from(split_command_line(example))
            .unwrap_or_else(|error| panic!("synopsis example '{example}' does not parse: {error}"));
    }
}

#[test]
fn every_flag_in_a_synopsis_form_exists_on_its_command() {
    let root = Cli::command();
    for (example, form) in synopsis_lines() {
        let name = example.split(' ').nth(1).unwrap();
        let command = root
            .find_subcommand(name)
            .unwrap_or_else(|| panic!("no command named {name}"));
        let known: Vec<&str> = command
            .get_arguments()
            .filter_map(|arg| arg.get_long())
            .collect();
        for flag in form.split(|c: char| !(c == '-' || c.is_ascii_alphanumeric())) {
            if let Some(long) = flag.strip_prefix("--") {
                assert!(
                    known.contains(&long),
                    "{name}: synopsis names --{long}, which the command lacks"
                );
            }
        }
    }
}

#[test]
fn every_flag_of_a_synopsis_command_appears_in_its_form() {
    let root = Cli::command();
    for (example, form) in synopsis_lines() {
        let name = example.split(' ').nth(1).unwrap();
        let command = root
            .find_subcommand(name)
            .unwrap_or_else(|| panic!("no command named {name}"));
        let named: Vec<&str> = form
            .split(|c: char| !(c == '-' || c.is_ascii_alphanumeric()))
            .filter_map(|word| word.strip_prefix("--"))
            .collect();
        for arg in command.get_arguments() {
            if arg.is_global_set() || arg.is_hide_set() {
                continue;
            }
            if let Some(long) = arg.get_long() {
                assert!(
                    named.contains(&long),
                    "{name}: synopsis form leaves out --{long}"
                );
            }
        }
    }
}
