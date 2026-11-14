use crate::system::app::alias_cache::AliasCache;
use crate::system::launch::indexed_entry::IndexedEntry;
use std::path::PathBuf;
use std::sync::Arc;

use crate::system::launch::desktop_entry::parse;
use crate::system::launch::desktop_exec::{ExecContext, expand};
use crate::system::launch::desktop_lookup::{current_dirs, find};
use crate::system::process::executable_identity::ExecutableIdentity;

/// The desktop entries of the machine, as the names each executable goes by.
pub(crate) struct AliasIndex {
    entries: Vec<IndexedEntry>,
}

impl AliasIndex {
    pub(in crate::system) fn build(files: &[(String, String)], dirs: &[PathBuf]) -> Self {
        let entries = files
            .iter()
            .filter_map(|(id, text)| parse(id, text))
            .filter(|entry| entry.traits.application && !entry.traits.hidden)
            .map(|entry| {
                let argv = entry
                    .exec
                    .as_deref()
                    .map(|exec| {
                        let file =
                            find(&entry.id, dirs).map(|file| file.to_string_lossy().into_owned());
                        if exec.contains("%k") && file.is_none() {
                            return Vec::new();
                        }
                        expand(
                            exec,
                            &ExecContext {
                                arguments: &[],
                                icon: entry.icon.as_deref(),
                                name: &entry.name,
                                file: file.as_deref(),
                            },
                        )
                    })
                    .unwrap_or_default();
                IndexedEntry::new(entry.id, entry.name, argv, entry.path)
            })
            .collect();
        Self { entries }
    }

    #[cfg(test)]
    pub(crate) fn names_for(&self, actual: &ExecutableIdentity) -> Vec<String> {
        self.names_and_ids(actual).0
    }

    /// The desktop ids of the entries that run the verified process's
    /// command, in byte order.
    pub(crate) fn ids_for(&self, actual: &ExecutableIdentity) -> Vec<String> {
        let path = crate::system::launch::launch_search::Search::from_env().path_dirs;
        self.matching_entries(actual, &path)
            .into_iter()
            .map(|entry| entry.id.clone())
            .collect()
    }

    /// The names and desktop ids the verified process answers to, from one
    /// pass over the entries. Aliases require desktop entries whose resolved
    /// executable and full argv match the process. Several entries that run
    /// that same command each name the process. A name is an alias only when
    /// no entry with that name runs another command. The ids are in byte
    /// order.
    pub(crate) fn names_and_ids(&self, actual: &ExecutableIdentity) -> (Vec<String>, Vec<String>) {
        let path = crate::system::launch::launch_search::Search::from_env().path_dirs;
        let matching = self.matching_entries(actual, &path);
        let ids = matching.iter().map(|entry| entry.id.clone()).collect();
        let mut names: Vec<String> = Vec::new();
        for entry in &matching {
            if !names
                .iter()
                .any(|name| name.eq_ignore_ascii_case(&entry.name))
                && self.name_is_unshared(entry, &path)
            {
                names.push(entry.name.clone());
            }
        }
        (names, ids)
    }

    /// Whether every other entry with this entry's `Name` runs the same
    /// command, so the name can stand for the process. The entry itself is
    /// not resolved against itself.
    pub(in crate::system) fn name_is_unshared(
        &self,
        entry: &IndexedEntry,
        path: &[PathBuf],
    ) -> bool {
        self.entries
            .iter()
            .filter(|other| {
                !std::ptr::eq(*other, entry) && other.name.eq_ignore_ascii_case(&entry.name)
            })
            .all(|other| other.runs_same_command(entry, path))
    }

    /// The entries whose command is the verified process's, sorted by
    /// desktop id. Each one runs the process's executable with its arguments,
    /// so they all run one command. Only entries whose words can run the
    /// process are resolved.
    fn matching_entries(
        &self,
        actual: &ExecutableIdentity,
        path: &[PathBuf],
    ) -> Vec<&IndexedEntry> {
        let mut matching: Vec<&IndexedEntry> = self
            .entries
            .iter()
            .filter(|entry| {
                actual.could_be_run_by(&entry.argv)
                    && entry
                        .resolved(path)
                        .is_some_and(|wanted| wanted.matches(actual))
            })
            .collect();
        matching.sort_by(|left, right| left.id.cmp(&right.id));
        matching
    }

    /// The entry with this desktop id.
    pub(in crate::system) fn entry(&self, desktop_id: &str) -> Option<&IndexedEntry> {
        self.entries.iter().find(|entry| entry.id == desktop_id)
    }

    /// Every entry whose `Name` equals `name`, ignoring case.
    pub(in crate::system) fn entries_named(&self, name: &str) -> Vec<&IndexedEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.name.eq_ignore_ascii_case(name))
            .collect()
    }
}

static CACHE: AliasCache = AliasCache::new();

/// The index of the entries the process environment names.
pub(crate) fn current_index() -> Arc<AliasIndex> {
    CACHE.index(&current_dirs())
}

#[cfg(test)]
#[path = "app_alias_tests.rs"]
mod tests;
