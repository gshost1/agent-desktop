use std::path::{Path, PathBuf};

use agent_desktop_core::WindowInfo;
use serde_json::Value;

use super::remembered_place::RememberedPlace;
use crate::system::app::runtime_store::{instance_stem, runtime_subdir, write_private_atomically};
use crate::system::process::{process_facts, process_lookup::Lookup};

const PLACE_DIR: &str = "agent-desktop-window-places";
const STALE_PENDING: std::time::Duration = std::time::Duration::from_secs(60);

/// The places windows had before the agent maximized them, kept in a private
/// directory of the runtime dir (mode 0700, files 0600) so a later process
/// can give them back. An entry is keyed by the window id and the start-time
/// token of the owning process, and both are checked again on every load, so
/// a reused window id never matches; entries of processes that are gone are
/// dropped on every save. A loaded entry is still only a claim about the
/// window: callers check it against the live window before acting on it.
pub(super) struct PlaceStore {
    dir: PathBuf,
}

impl PlaceStore {
    pub(super) fn in_runtime_dir() -> Option<Self> {
        runtime_subdir(PLACE_DIR).map(Self::at)
    }

    pub(super) fn at(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn path(&self, window: u32, instance: &str) -> PathBuf {
        self.dir
            .join(format!("{}.json", instance_stem(window, instance)))
    }

    /// Records `place` for the window of `win`, or drops its record when
    /// `place` is `None`. A window listed without a process instance has no
    /// record.
    pub(super) fn remember(win: &WindowInfo, client: u32, place: Option<RememberedPlace>) {
        let (Some(store), Some(instance)) =
            (Self::in_runtime_dir(), win.process_instance.as_deref())
        else {
            return;
        };
        match place {
            Some(place) => {
                let alive = |pid: u32, instance: &str| matches!(process_facts::lookup(pid), Lookup::Found(facts) if facts.instance == instance);
                let _ = store.save(client, win.pid.get(), instance, place, &alive);
            }
            None => store.forget(client, instance),
        }
    }

    pub(super) fn recall(win: &WindowInfo, client: u32) -> Option<RememberedPlace> {
        Self::in_runtime_dir()?.load(client, win.process_instance.as_deref()?)
    }

    pub(super) fn save(
        &self,
        window: u32,
        pid: u32,
        instance: &str,
        place: RememberedPlace,
        alive: &dyn Fn(u32, &str) -> bool,
    ) -> std::io::Result<()> {
        self.sweep(alive);
        let mut record = place.to_json();
        record["pid"] = pid.into();
        record["instance"] = instance.into();
        record["window"] = window.into();
        write_private_atomically(
            &self.dir,
            &self.path(window, instance),
            record.to_string().as_bytes(),
        )
    }

    /// The record of the window, checked against the window id and process
    /// instance inside the entry. An entry that cannot be read as a record of
    /// this window can never be used, so it is deleted.
    pub(super) fn load(&self, window: u32, instance: &str) -> Option<RememberedPlace> {
        let path = self.path(window, instance);
        let bytes = std::fs::read(&path).ok()?;
        let record = serde_json::from_slice::<Value>(&bytes)
            .ok()
            .filter(|value| {
                value["instance"].as_str() == Some(instance)
                    && value["window"].as_u64() == Some(u64::from(window))
            })
            .and_then(|value| RememberedPlace::from_json(&value));
        if record.is_none() {
            let _ = std::fs::remove_file(path);
        }
        record
    }

    pub(super) fn forget(&self, window: u32, instance: &str) {
        let _ = std::fs::remove_file(self.path(window, instance));
    }

    fn sweep(&self, alive: &dyn Fn(u32, &str) -> bool) {
        let Ok(listing) = std::fs::read_dir(&self.dir) else {
            return;
        };
        for file in listing.flatten() {
            let path = file.path();
            if is_stale_pending(&path) {
                let _ = std::fs::remove_file(&path);
                continue;
            }
            if !is_record(&path) {
                continue;
            }
            let owner = std::fs::read(&path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                .and_then(|value| {
                    let pid = u32::try_from(value["pid"].as_u64()?).ok()?;
                    Some((pid, value["instance"].as_str()?.to_owned()))
                });
            if owner.is_none_or(|(pid, instance)| !alive(pid, &instance)) {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

fn is_stale_pending(path: &Path) -> bool {
    path.extension().is_some_and(|extension| extension == "tmp")
        && std::fs::metadata(path)
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|age| age >= STALE_PENDING)
}

fn is_record(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == "json")
}

#[cfg(test)]
#[path = "window_place_tests.rs"]
mod tests;
