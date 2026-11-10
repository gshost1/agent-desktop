use std::path::{Path, PathBuf};

use super::runtime_store::{named_stem, runtime_subdir, write_private_atomically};
use crate::system::process::running_app::Running;

const ENTRIES_DIR: &str = "agent-desktop-launched-entries";

/// How many records the store keeps; the oldest go first.
const KEPT: usize = 256;

/// The desktop entry that `launch` started each process from, kept per
/// process instance in a private directory of the runtime dir. A launcher
/// that `exec`s another binary leaves a process whose executable no entry's
/// `Exec` names, and launch arguments make its argv differ from every entry,
/// so this record is the only evidence that ties it to the entry. A record
/// answers only for the executable the process ran when it was recorded.
pub(in crate::system) struct LaunchedEntries {
    dir: PathBuf,
}

impl LaunchedEntries {
    pub(in crate::system) fn in_runtime_dir() -> Option<Self> {
        runtime_subdir(ENTRIES_DIR).map(|dir| Self { dir })
    }

    fn path(&self, instance: &str) -> PathBuf {
        self.dir.join(named_stem("entry", instance))
    }

    pub(in crate::system) fn remember(&self, instance: &str, executable: &Path, desktop_id: &str) {
        let text = format!("{}\n{desktop_id}", executable.display());
        if write_private_atomically(&self.dir, &self.path(instance), text.as_bytes()).is_ok() {
            self.prune();
        }
    }

    pub(in crate::system) fn recall(&self, instance: &str, executable: &Path) -> Option<String> {
        let text = std::fs::read_to_string(self.path(instance)).ok()?;
        let (recorded, desktop_id) = text.rsplit_once('\n')?;
        (recorded == executable.display().to_string() && !desktop_id.is_empty())
            .then(|| desktop_id.to_owned())
    }

    fn prune(&self) {
        let Ok(listing) = std::fs::read_dir(&self.dir) else {
            return;
        };
        let mut records: Vec<(std::time::SystemTime, PathBuf)> = listing
            .flatten()
            .filter_map(|entry| {
                let modified = entry.metadata().ok()?.modified().ok()?;
                Some((modified, entry.path()))
            })
            .collect();
        if records.len() <= KEPT {
            return;
        }
        records.sort();
        let excess = records.len() - KEPT;
        for (_, path) in records.into_iter().take(excess) {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Records the desktop entry a launch started its owner from, when the owner
/// is a process the launch created.
pub(in crate::system) fn remember_launch(desktop_id: Option<&str>, owner: &Running) {
    let (Some(desktop_id), Some(executable)) = (desktop_id, owner.executable.as_ref()) else {
        return;
    };
    if let Some(store) = LaunchedEntries::in_runtime_dir() {
        store.remember(&owner.instance, &executable.executable, desktop_id);
    }
}

#[cfg(test)]
#[path = "launched_entries_tests.rs"]
mod tests;
