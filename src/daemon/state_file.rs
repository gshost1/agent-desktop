//! `<key>.state`: the daemon's pid and how many commands it has served, rewritten after each
//! one. It lets an operator stop a daemon and lets tests see which process ran a command.

use std::fs::File;
use std::os::unix::fs::{FileExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

/// The open state file of a running daemon; removed when the daemon exits.
pub(crate) struct DaemonStateFile {
    file: File,
    path: PathBuf,
}

impl DaemonStateFile {
    pub(crate) fn create(path: &Path) -> std::io::Result<Self> {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)?;
        let state = Self {
            file,
            path: path.to_path_buf(),
        };
        state.record(0)?;
        Ok(state)
    }

    /// Writes the current count; a reader sees either the old or the new line.
    pub(crate) fn record(&self, served: u64) -> std::io::Result<()> {
        let line = format!(
            "{{\"pid\":{},\"served\":{served:>20}}}\n",
            std::process::id()
        );
        self.file.write_all_at(line.as_bytes(), 0)
    }

    pub(crate) fn remove(&self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
#[path = "state_file_tests.rs"]
mod tests;
