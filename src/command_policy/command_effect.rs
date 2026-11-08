//! The widest effect a command can have outside this process. It decides how
//! a failure that carries no delivery evidence is settled, so a command that
//! only reads, or only writes a local file, is never reported as possibly
//! delivered to an application.

use crate::cli::Commands;

/// What a command can change. Mutating commands that are not listed as local
/// writes default to [`CommandEffect::AppInput`], which never claims that
/// nothing was delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommandEffect {
    /// Reads only: nothing is sent and nothing is written.
    ReadOnly,
    /// Writes a local file or local state and sends nothing to an application.
    LocalWrite,
    /// May send input to an application or change state other applications
    /// see, such as the clipboard or a window.
    AppInput,
}

impl CommandEffect {
    pub(crate) fn of(cmd: &Commands) -> Self {
        if !cmd.is_mutating() {
            return Self::ReadOnly;
        }
        match cmd {
            Commands::Screenshot(_)
            | Commands::ClipboardGet(_)
            | Commands::Session(_)
            | Commands::Trace(_) => Self::LocalWrite,
            _ => Self::AppInput,
        }
    }

    pub(crate) fn sends_input(self) -> bool {
        self == Self::AppInput
    }
}

#[cfg(test)]
#[path = "command_effect_tests.rs"]
mod tests;
