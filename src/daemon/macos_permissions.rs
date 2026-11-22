//! The macOS CLI checks its own grants before it connects to a daemon.

use agent_desktop_core::commands::batch::parse_commands;
use clap::Parser;

use crate::cli::{Cli, Commands};
use crate::command_policy::{policy_for, requires_accessibility, requires_screen_recording};

pub(super) fn permits_forwarding(
    argv: &[String],
    session_from_env: bool,
    check_accessibility: impl FnOnce() -> bool,
    check_screen_recording: impl FnOnce() -> bool,
) -> bool {
    let Ok(cli) = Cli::try_parse_from(argv) else {
        return false;
    };
    let Some(command) = cli.command else {
        return false;
    };
    let in_session = session_from_env || cli.identity.session.is_some();
    let Some((accessibility, screen_recording)) = required_grants(&command, in_session) else {
        return false;
    };
    (!accessibility || check_accessibility())
        && (!(screen_recording || cli.visual_debug.debug) || check_screen_recording())
}

fn required_grants(command: &Commands, in_session: bool) -> Option<(bool, bool)> {
    match command {
        Commands::Batch(args) => {
            let entries = parse_commands(&args.commands_json).ok()?;
            return entries
                .into_iter()
                .try_fold((false, false), |needed, entry| {
                    let in_session = in_session || entry.session.is_some();
                    let command = crate::batch::parse_command(entry).ok()?;
                    let (accessibility, screen_recording) = required_grants(&command, in_session)?;
                    Some((needed.0 || accessibility, needed.1 || screen_recording))
                });
        }
        Commands::ListWindows(_) => return Some((true, true)),
        Commands::Status | Commands::Permissions(_) => return None,
        _ => {}
    }
    let policy = policy_for(command);
    let accessibility = requires_accessibility(policy);
    let may_capture_session_artifacts =
        in_session && accessibility && command.effect().sends_input();
    let screen_recording = requires_screen_recording(policy) || may_capture_session_artifacts;
    Some((accessibility, screen_recording))
}

#[cfg(test)]
#[path = "macos_permissions_tests.rs"]
mod tests;
