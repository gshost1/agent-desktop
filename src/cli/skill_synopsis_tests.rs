use crate::cli::Cli;
use clap::{CommandFactory, Parser};

const INTERACTION_REFERENCE: &str =
    include_str!("../../skills/agent-desktop/references/commands-interaction.md");

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
    let section = INTERACTION_REFERENCE
        .split_once("## Command synopsis")
        .expect("the interaction reference has a command synopsis")
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
