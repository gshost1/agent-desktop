use clap::Parser;

use super::CommandEffect;
use crate::cli::Cli;

fn effect(arguments: &[&str]) -> CommandEffect {
    let argv = std::iter::once("agent-desktop").chain(arguments.iter().copied());
    let command = Cli::try_parse_from(argv)
        .expect("command parses")
        .command
        .expect("subcommand is present");
    CommandEffect::of(&command)
}

#[test]
fn commands_that_write_only_local_files_or_state_send_no_input() {
    for arguments in [
        &["screenshot", "/tmp/capture.png"][..],
        &["clipboard-get", "--out", "/tmp/clipboard.png"],
        &["session", "start"],
        &["session", "gc"],
        &["trace", "export"],
    ] {
        assert_eq!(
            effect(arguments),
            CommandEffect::LocalWrite,
            "{arguments:?}"
        );
    }
}

#[test]
fn read_only_commands_and_their_inline_forms_are_read_only() {
    for arguments in [
        &["screenshot"][..],
        &["clipboard-get"],
        &["session", "list"],
        &["trace", "show"],
        &["list-windows"],
        &["permissions"],
    ] {
        assert_eq!(effect(arguments), CommandEffect::ReadOnly, "{arguments:?}");
    }
}

#[test]
fn commands_that_reach_applications_or_shared_state_are_app_input() {
    for arguments in [
        &["click", "@e1"][..],
        &["press", "cmd+c"],
        &["clipboard-clear"],
        &["permissions", "--request"],
        &["cursor-overlay", "disable"],
        &["batch", "[]"],
    ] {
        assert_eq!(effect(arguments), CommandEffect::AppInput, "{arguments:?}");
    }
}
