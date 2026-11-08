//! How a tool call's child stands toward an interaction lease the server inherited.

use clap::Parser;

use crate::cli::{Cli, Commands};
use crate::command_policy::{PermissionNeed, policy_for};

/// `Exclusive` for a call that can inject input or steal focus: it changes something, reads the permission
/// report (which can release stuck keys), or reads notifications with `--headed`. It holds the lease alone,
/// one at a time in arrival order. `Shared` for a read that may take the lease itself, such as renderer
/// accessibility activation: it injects no input, so it gets the lease beside the holder. `Unused` for a
/// command that never takes the lease and needs no platform permission: its child gets nothing.
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
        let steals_focus = headed && reads_notifications(command);
        if command.is_mutating()
            || steals_focus
            || matches!(command, Commands::Status | Commands::Permissions(_))
        {
            Self::Exclusive
        } else if policy_for(command) == PermissionNeed::None {
            Self::Unused
        } else {
            Self::Shared
        }
    }
}

fn reads_notifications(command: &Commands) -> bool {
    match command {
        Commands::ListNotifications(_) => true,
        Commands::Wait(args) => args.mode.notification,
        _ => false,
    }
}
