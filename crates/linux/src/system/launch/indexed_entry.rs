use std::path::{Path, PathBuf};

use crate::system::process::executable_identity::ExecutableIdentity;

pub(in crate::system) struct IndexedEntry {
    pub(in crate::system) id: String,
    pub(in crate::system) name: String,
    pub(in crate::system) argv: Vec<String>,
    pub(in crate::system) cwd: Option<String>,
    /// The expanded commands of the entry's actions and of its bus
    /// activation, which start the same application with other arguments.
    pub(in crate::system) other_commands: Vec<Vec<String>>,
}

#[cfg(test)]
thread_local! {
    static RESOLUTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many entry executables this thread resolved, for tests that bound
/// the lookup cost.
#[cfg(test)]
pub(in crate::system) fn resolutions() -> usize {
    RESOLUTIONS.with(std::cell::Cell::get)
}

fn resolve(argv: &[String], path: &[PathBuf]) -> Option<ExecutableIdentity> {
    #[cfg(test)]
    RESOLUTIONS.with(|count| count.set(count.get() + 1));
    ExecutableIdentity::requested(argv, path)
}

impl IndexedEntry {
    pub(in crate::system) fn new(
        id: String,
        name: String,
        argv: Vec<String>,
        cwd: Option<String>,
    ) -> Self {
        Self {
            id,
            name,
            argv,
            cwd,
            other_commands: Vec::new(),
        }
    }

    /// The entry with the other commands it starts its application with.
    pub(in crate::system) fn with_other_commands(mut self, commands: Vec<Vec<String>>) -> Self {
        self.other_commands = commands
            .into_iter()
            .filter(|command| !command.is_empty() && *command != self.argv)
            .collect();
        self
    }

    /// Whether the verified process runs the entry's command or one of its
    /// other commands. Only a command whose words can run the process is
    /// resolved.
    pub(in crate::system) fn runs(&self, actual: &ExecutableIdentity, path: &[PathBuf]) -> bool {
        std::iter::once(&self.argv)
            .chain(&self.other_commands)
            .filter(|command| actual.could_be_run_by(command))
            .any(|command| resolve(command, path).is_some_and(|wanted| wanted.matches(actual)))
    }

    /// The entry's command as an executable identity, resolved through
    /// `path` and canonicalized now; nothing is kept, so a lookup always sees
    /// the current PATH and filesystem.
    pub(in crate::system) fn resolved(&self, path: &[PathBuf]) -> Option<ExecutableIdentity> {
        resolve(&self.argv, path)
    }

    /// Whether two entries run the same command: one canonical executable
    /// reached under the same invocation name, so a multi-call binary that
    /// dispatches on its name counts once per name, with the same arguments
    /// and the same working directory. A program that cannot be resolved
    /// counts as the same only when it is spelled the same way.
    pub(in crate::system) fn runs_same_command(&self, other: &Self, path: &[PathBuf]) -> bool {
        let (Some((program, arguments)), Some((other_program, other_arguments))) =
            (self.argv.split_first(), other.argv.split_first())
        else {
            return false;
        };
        if self.cwd != other.cwd
            || arguments != other_arguments
            || Path::new(program).file_name() != Path::new(other_program).file_name()
        {
            return false;
        }
        match (self.resolved(path), other.resolved(path)) {
            (Some(mine), Some(theirs)) => mine.executable == theirs.executable,
            (None, None) => program == other_program,
            _ => false,
        }
    }
}
