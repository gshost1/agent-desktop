use std::path::{Path, PathBuf};

use super::runtime_store::{instance_stem, runtime_subdir, write_private_atomically};
use crate::system::process::process_facts;
use crate::system::process::running_app::Running;

const ENTRIES_DIR: &str = "agent-desktop-launched-entries";
const RECORD_EXTENSION: &str = "entry";

/// How many records the store keeps after the records of processes that are
/// gone are dropped; the oldest go first.
const KEPT: usize = 256;

/// The desktop entry that `launch` started each process from, kept per pid
/// and process instance in a private directory of the runtime dir. A
/// launcher that `exec`s another binary leaves a process whose executable no
/// entry's `Exec` names, and launch arguments make its argv differ from every
/// entry, so this record is the only evidence that ties it to the entry. A
/// record answers only for the pid, instance and executable it was written
/// for.
pub(in crate::system) struct LaunchedEntries {
    dir: PathBuf,
}

impl LaunchedEntries {
    pub(in crate::system) fn in_runtime_dir() -> Option<Self> {
        runtime_subdir(ENTRIES_DIR).map(|dir| Self { dir })
    }

    fn path(&self, pid: u32, instance: &str) -> PathBuf {
        self.dir
            .join(instance_stem(pid, instance))
            .with_extension(RECORD_EXTENSION)
    }

    pub(in crate::system) fn remember(
        &self,
        owner: (u32, &str),
        executable: &Path,
        desktop_id: &str,
    ) {
        let path = self.path(owner.0, owner.1);
        let text = format!("{}\n{}\n{desktop_id}", owner.0, executable.display());
        if write_private_atomically(&self.dir, &path, text.as_bytes()).is_ok() {
            self.prune(&path, record_is_gone);
        }
    }

    pub(in crate::system) fn recall(
        &self,
        pid: u32,
        instance: &str,
        executable: &Path,
    ) -> Option<String> {
        let text = std::fs::read_to_string(self.path(pid, instance)).ok()?;
        let (recorded, desktop_id) = text.split_once('\n')?.1.rsplit_once('\n')?;
        (recorded == executable.display().to_string() && !desktop_id.is_empty())
            .then(|| desktop_id.to_owned())
    }

    /// Drops the records of processes that are gone, then the oldest beyond
    /// [`KEPT`]. Pending files of other writers and the record just written
    /// are never touched.
    fn prune(&self, written: &Path, is_gone: impl Fn(&Path) -> bool) {
        let Ok(listing) = std::fs::read_dir(&self.dir) else {
            return;
        };
        let mut kept: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
        for entry in listing.flatten() {
            let path = entry.path();
            if path == written
                || path.extension().and_then(|ext| ext.to_str()) != Some(RECORD_EXTENSION)
            {
                continue;
            }
            if is_gone(&path) {
                let _ = std::fs::remove_file(&path);
            } else if let Ok(modified) = entry.metadata().and_then(|meta| meta.modified()) {
                kept.push((modified, path));
            }
        }
        if kept.len() < KEPT {
            return;
        }
        kept.sort();
        let excess = kept.len() + 1 - KEPT;
        for (_, path) in kept.into_iter().take(excess) {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Whether the process a record names has exited: its pid answers no more,
/// or a different process instance now holds it.
fn record_is_gone(path: &Path) -> bool {
    let Some(pid) = std::fs::read_to_string(path).ok().and_then(|text| {
        text.split_once('\n')
            .and_then(|(pid, _)| pid.parse::<u32>().ok())
    }) else {
        return false;
    };
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    process_facts::read(pid).is_none_or(|facts| stem != instance_stem(pid, &facts.instance))
}

/// Records the desktop entry a launch started its owner from when the owner
/// is the very process the launch spawned, so a launcher that `exec`ed the
/// application is covered and a process the launch only handed off to is not.
pub(in crate::system) fn remember_launch(
    desktop_id: Option<&str>,
    owner: &Running,
    spawned_pid: u32,
) {
    let (Some(desktop_id), Some(executable)) = (desktop_id, owner.executable.as_ref()) else {
        return;
    };
    if owner.pid != spawned_pid {
        return;
    }
    if let Some(store) = LaunchedEntries::in_runtime_dir() {
        store.remember(
            (owner.pid, &owner.instance),
            &executable.executable,
            desktop_id,
        );
    }
}

#[cfg(test)]
#[path = "launched_entries_tests.rs"]
mod tests;
