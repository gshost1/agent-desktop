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
                IndexedEntry {
                    id: entry.id,
                    name: entry.name,
                    argv,
                }
            })
            .collect();
        Self { entries }
    }

    /// Aliases require desktop entries whose resolved executable and full
    /// argv match the verified process. Several entries that run that same
    /// command each name the process. A name is an alias only when no entry
    /// with that name runs another command.
    pub(crate) fn names_for(&self, actual: &ExecutableIdentity) -> Vec<String> {
        let path = crate::system::launch::launch_search::Search::from_env().path_dirs;
        let matching = self.matching_entries(actual, &path);
        let mut names: Vec<String> = Vec::new();
        for entry in &matching {
            let shared_only_by_this_command = self
                .entries
                .iter()
                .filter(|other| other.name.eq_ignore_ascii_case(&entry.name))
                .all(|other| same_command(other, entry, &path));
            if shared_only_by_this_command
                && !names
                    .iter()
                    .any(|name| name.eq_ignore_ascii_case(&entry.name))
            {
                names.push(entry.name.clone());
            }
        }
        names
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

    /// The entries whose command is the verified process's, sorted by
    /// desktop id. Entries that match it but do not run one command between
    /// them name no process.
    fn matching_entries(
        &self,
        actual: &ExecutableIdentity,
        path: &[PathBuf],
    ) -> Vec<&IndexedEntry> {
        let mut matching: Vec<&IndexedEntry> = self
            .entries
            .iter()
            .filter(|entry| {
                ExecutableIdentity::requested(&entry.argv, path)
                    .is_some_and(|wanted| wanted.matches(actual))
            })
            .collect();
        matching.sort_by(|left, right| left.id.cmp(&right.id));
        let one_command = matching.first().is_some_and(|first| {
            matching
                .iter()
                .all(|entry| same_command(entry, first, path))
        });
        if one_command { matching } else { Vec::new() }
    }

    /// The id of every entry whose `Name` equals `name`, ignoring case.
    pub(crate) fn ids_named(&self, name: &str) -> Vec<String> {
        self.entries
            .iter()
            .filter(|entry| entry.name.eq_ignore_ascii_case(name))
            .map(|entry| entry.id.clone())
            .collect()
    }
}

fn same_command(left: &IndexedEntry, right: &IndexedEntry, path: &[PathBuf]) -> bool {
    ExecutableIdentity::requested(&left.argv, path)
        .zip(ExecutableIdentity::requested(&right.argv, path))
        .is_some_and(|(left, right)| left.runs_same_command(&right))
}

static CACHE: AliasCache = AliasCache::new();

/// The index of the entries the process environment names.
pub(crate) fn current_index() -> Arc<AliasIndex> {
    CACHE.index(&current_dirs())
}

#[cfg(test)]
#[path = "app_alias_tests.rs"]
mod tests;
