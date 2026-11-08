//! The widest effect a command can have outside this process, decided once
//! per command by `Commands::effect`. It settles a failure that carries no
//! delivery evidence, so a command that only reads, or only writes a local
//! file, is never reported as possibly delivered to an application.

/// What a command can change.
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
    pub(crate) fn sends_input(self) -> bool {
        self == Self::AppInput
    }
}

#[cfg(test)]
#[path = "command_effect_tests.rs"]
mod tests;
