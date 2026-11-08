use super::{Cli, Commands};
use agent_desktop_core::commands::batch::BatchCommand;
use clap::{CommandFactory, Parser};

const DESKTOP_SKILL: &str = include_str!("../../skills/agent-desktop/SKILL.md");
const MACOS_SKILL: &str = include_str!("../../skills/agent-desktop-macos/SKILL.md");

fn fenced_bash_lines(skill: &'static str) -> Vec<&'static str> {
    skill
        .split("```bash\n")
        .skip(1)
        .filter_map(|rest| rest.split("```").next())
        .flat_map(str::lines)
        .filter(|line| line.starts_with("agent-desktop "))
        .collect()
}

fn inline_code_spans(skill: &'static str) -> Vec<&'static str> {
    let mut spans = Vec::new();
    let mut in_fence = false;
    for line in skill.lines() {
        if line.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        spans.extend(line.split('`').skip(1).step_by(2));
    }
    spans
}

fn subcommand_names() -> Vec<String> {
    Cli::command()
        .get_subcommands()
        .map(|command| command.get_name().to_string())
        .collect()
}

fn command_lines(skill: &'static str) -> Vec<String> {
    let subcommands = subcommand_names();
    let mut lines: Vec<String> = fenced_bash_lines(skill)
        .into_iter()
        .map(str::to_string)
        .collect();
    for span in inline_code_spans(skill) {
        if span.starts_with("agent-desktop ") {
            lines.push(span.to_string());
            continue;
        }
        let Some((first, _)) = span.split_once(' ') else {
            continue;
        };
        if subcommands.iter().any(|name| name == first) {
            lines.push(format!("agent-desktop {span}"));
        }
    }
    lines
}

fn shell_words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote: Option<char> = None;
    let mut in_word = false;
    for ch in line.chars() {
        match (quote, ch) {
            (Some(open), c) if c == open => quote = None,
            (Some(_), c) => word.push(c),
            (None, '"' | '\'') => {
                quote = Some(ch);
                in_word = true;
            }
            (None, '#') if !in_word => break,
            (None, c) if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            (None, c) => {
                word.push(c);
                in_word = true;
            }
        }
    }
    assert!(quote.is_none(), "unclosed quote in quickstart line: {line}");
    if in_word {
        words.push(word);
    }
    words
}

fn assert_parses(skill_name: &str, line: &str) {
    let parsed = Cli::try_parse_from(shell_words(line)).unwrap_or_else(|error| {
        panic!("{skill_name} quickstart line does not parse: {line}\n{error}")
    });
    if let Some(Commands::Batch(args)) = parsed.command {
        let entries: Vec<BatchCommand> =
            serde_json::from_str(&args.commands_json).unwrap_or_else(|error| {
                panic!("{skill_name} quickstart batch JSON is invalid: {error}")
            });
        for entry in entries {
            crate::batch::parse_command(entry).unwrap_or_else(|error| {
                panic!("{skill_name} quickstart batch entry rejected: {error}")
            });
        }
    }
}

#[test]
fn every_desktop_quickstart_line_parses_with_the_shipped_cli() {
    assert!(
        fenced_bash_lines(DESKTOP_SKILL).len() >= 15,
        "the desktop quickstart block lost its command lines"
    );
    for line in command_lines(DESKTOP_SKILL) {
        assert_parses("desktop", &line);
    }
}

#[test]
fn every_macos_quickstart_line_parses_with_the_shipped_cli() {
    let lines = command_lines(MACOS_SKILL);
    assert!(
        lines.len() >= 5,
        "the macOS quickstart lost its command lines: {lines:?}"
    );
    for line in lines {
        assert_parses("macOS", &line);
    }
}

#[test]
fn inline_spans_that_name_a_subcommand_become_command_lines() {
    let lines = command_lines("Use `press cmd+s --app \"TextEdit\"`, `close-app` and `--headed`.");
    assert_eq!(lines, ["agent-desktop press cmd+s --app \"TextEdit\""]);
}

#[test]
fn the_shell_word_splitter_honors_quotes_and_comments() {
    assert_eq!(
        shell_words(r#"agent-desktop find --name "Save As" 'a b'  # note"#),
        ["agent-desktop", "find", "--name", "Save As", "a b"]
    );
}
