//! How a tool call's child stands toward an interaction lease the server inherited.

use clap::Parser;

use crate::cli::{Cli, Commands};
use crate::command_policy::requires_permission_report;

/// `Exclusive` for a call that can inject input or steal focus: it changes something (`permissions
/// --request` included) or reads notifications with `--headed`. It holds the lease alone, one at a time in
/// arrival order. `Shared` for a read that may take the lease itself, such as the permission report or
/// renderer accessibility activation, so it gets the lease beside the holder. A shared read sends no input
/// under it: the one input a read can send, the stale-input sweep of the permission report, runs only under
/// a lease its process owns, never under an adopted one. `Unused` for a command that reads no permission
/// report and so never takes the lease: its child gets nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LeaseRole {
    Exclusive,
    Shared,
    Unused,
}

impl LeaseRole {
    /// The role of running `argv` after the server's `globals`; anything that does not parse is `Exclusive`.
    pub(crate) fn of_call(globals: &[String], argv: &[String]) -> Self {
        let words = std::iter::once("agent-desktop")
            .chain(globals.iter().map(String::as_str))
            .chain(argv.iter().map(String::as_str));
        match Cli::try_parse_from(words) {
            Ok(Cli {
                command: Some(command),
                interaction,
                ..
            }) => Self::of(&command, interaction.headed),
            _ => Self::Exclusive,
        }
    }

    fn of(command: &Commands, headed: bool) -> Self {
        if command.is_mutating() || (headed && reads_notifications(command)) {
            Self::Exclusive
        } else if requires_permission_report(command) {
            Self::Shared
        } else {
            Self::Unused
        }
    }
}

fn reads_notifications(command: &Commands) -> bool {
    match command {
        Commands::ListNotifications(_) => true,
        Commands::Wait(args) => args.surface.notification,
        _ => false,
    }
}

#[cfg(test)]
#[path = "lease_role_tests.rs"]
mod tests;
