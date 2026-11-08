use super::{Cli, Commands};
use agent_desktop_core::commands::batch::BatchCommand;
use clap::Parser;

const DESKTOP_SKILL: &str = include_str!("../../skills/agent-desktop/SKILL.md");

fn quickstart_lines() -> Vec<&'static str> {
    let block = DESKTOP_SKILL
        .split("```bash\n")
        .nth(1)
        .expect("the desktop skill opens with a bash quickstart block")
        .split("```")
        .next()
        .unwrap_or_default();
    block
        .lines()
        .filter(|line| line.starts_with("agent-desktop "))
        .collect()
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

#[test]
fn every_quickstart_line_parses_with_the_shipped_cli() {
    let lines = quickstart_lines();
    assert!(
        lines.len() >= 15,
        "the quickstart block lost its command lines"
    );
    for line in lines {
        let parsed = Cli::try_parse_from(shell_words(line))
            .unwrap_or_else(|error| panic!("quickstart line does not parse: {line}\n{error}"));
        if let Some(Commands::Batch(args)) = parsed.command {
            let entries: Vec<BatchCommand> = serde_json::from_str(&args.commands_json)
                .unwrap_or_else(|error| panic!("quickstart batch JSON is invalid: {error}"));
            for entry in entries {
                crate::batch::parse_command(entry)
                    .unwrap_or_else(|error| panic!("quickstart batch entry rejected: {error}"));
            }
        }
    }
}

#[test]
fn the_shell_word_splitter_honors_quotes_and_comments() {
    assert_eq!(
        shell_words(r#"agent-desktop find --name "Save As" 'a b'  # note"#),
        ["agent-desktop", "find", "--name", "Save As", "a b"]
    );
}
