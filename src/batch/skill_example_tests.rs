use super::parse_command;
use agent_desktop_core::commands::batch::{BatchCommand, parse_commands};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const BATCH_PREFIX: &str = "agent-desktop batch '";

fn skill_files(dir: &Path, found: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            skill_files(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "md") {
            found.push(path);
        }
    }
}

fn batch_examples() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../skills");
    let mut files = Vec::new();
    skill_files(&root, &mut files);
    let mut examples = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        let mut rest = text.as_str();
        while let Some(start) = rest.find(BATCH_PREFIX) {
            let body = &rest[start + BATCH_PREFIX.len()..];
            let end = body
                .find('\'')
                .unwrap_or_else(|| panic!("unterminated batch example in {}", file.display()));
            examples.push((file.display().to_string(), body[..end].to_string()));
            rest = &body[end..];
        }
    }
    examples
}

fn item(command: &str, args: Value) -> BatchCommand {
    BatchCommand {
        command: command.to_string(),
        session: None,
        args,
    }
}

#[test]
fn every_batch_example_in_the_skills_parses() {
    let examples = batch_examples();

    assert!(
        examples.len() >= 6,
        "expected the shipped batch examples, found {}",
        examples.len()
    );
    for (file, json) in examples {
        let entries = parse_commands(&json)
            .unwrap_or_else(|error| panic!("{file}: batch JSON does not parse: {error}\n{json}"));
        assert!(!entries.is_empty(), "{file}: empty batch example");
        for entry in entries {
            let command = entry.command.clone();
            parse_command(entry).unwrap_or_else(|error| {
                panic!("{file}: batch entry '{command}' is rejected: {error}")
            });
        }
    }
}

#[test]
fn the_three_flags_named_apart_from_their_fields_take_the_field_name() {
    let cases = [
        (
            "press",
            json!({ "combo": "cmd+s", "ref_id": "@s8f3k2p9:e1" }),
            "ref",
        ),
        (
            "find",
            json!({ "app": "TextEdit", "states": ["focused"] }),
            "state",
        ),
        (
            "launch",
            json!({ "app": "TextEdit", "args": ["-x"] }),
            "arg",
        ),
    ];
    for (command, args, flag) in cases {
        parse_command(item(command, args.clone())).unwrap_or_else(|error| {
            panic!("{command} with the field name for --{flag} is rejected: {error}")
        });
        let mut flag_named = args.as_object().unwrap().clone();
        let field = flag_named
            .keys()
            .find(|key| key.as_str() != "app" && key.as_str() != "combo")
            .unwrap()
            .clone();
        let value = flag_named.remove(&field).unwrap();
        flag_named.insert(flag.to_string(), value);
        assert!(
            parse_command(item(command, Value::Object(flag_named))).is_err(),
            "{command} must reject the flag name '{flag}'"
        );
    }
}

#[test]
fn a_flag_whose_name_matches_its_field_drops_the_dashes() {
    parse_command(item(
        "find",
        json!({ "app": "TextEdit", "timeout_ms": 500, "role": "button" }),
    ))
    .unwrap();
}
