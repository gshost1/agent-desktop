use super::app_alias::{AliasIndex, current_index};
use super::launched_entries::LaunchedEntries;
use crate::system::process::process_facts::ProcessFacts;

fn non_blank(text: Option<&str>) -> Option<&str> {
    text.filter(|text| !text.trim().is_empty())
}

const DELETED_SUFFIX: &str = " (deleted)";

/// An executable's basename without the marker the kernel appends to the
/// link target of a binary that was removed or replaced while it runs, so an
/// application keeps its name across a package upgrade.
fn live_basename(basename: &str) -> &str {
    basename.strip_suffix(DELETED_SUFFIX).unwrap_or(basename)
}

/// The name an application is listed under: the executable's basename,
/// then `comm`, which the kernel cuts at 15 characters. It comes from `/proc`
/// alone, never from the accessibility root, so an application keeps one name
/// whether it answers or is stopped: core upgrades a stale reference to an
/// unresponsive application only when the listed name is unchanged.
pub(crate) fn app_name(facts: &ProcessFacts) -> String {
    non_blank(facts.exe_basename.as_deref())
        .map(live_basename)
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(&facts.comm)
        .to_owned()
}

/// The desktop entry names and ids the process's executable answers to. The
/// listed name never follows them.
pub(crate) fn aliases_of(pid: u32, facts: &ProcessFacts) -> Vec<String> {
    aliases_in(&current_index(), pid, facts)
}

/// The desktop entry `launch` recorded for this pid, process instance and
/// executable, when there is one.
fn launched_entry(pid: u32, facts: &ProcessFacts) -> Option<String> {
    let executable = &facts.executable.as_ref()?.executable;
    LaunchedEntries::in_runtime_dir()?.recall(pid, &facts.instance, executable)
}

pub(crate) fn desktop_id_in(index: &AliasIndex, pid: u32, facts: &ProcessFacts) -> Option<String> {
    desktop_id_with(index, facts, launched_entry(pid, facts))
}

/// The entry `launch` recorded for the process, else the first entry whose
/// command the process runs.
fn desktop_id_with(
    index: &AliasIndex,
    facts: &ProcessFacts,
    launched: Option<String>,
) -> Option<String> {
    launched
        .filter(|desktop_id| index.entry(desktop_id).is_some())
        .or_else(|| index.ids_for(facts.executable.as_ref()?).into_iter().next())
}

pub(crate) fn desktop_id(pid: u32, facts: &ProcessFacts) -> Option<String> {
    desktop_id_in(&current_index(), pid, facts)
}

/// As [`aliases_of`] over an index the caller already holds, for a caller that
/// asks about many processes.
pub(crate) fn aliases_in(index: &AliasIndex, pid: u32, facts: &ProcessFacts) -> Vec<String> {
    aliases_with(index, facts, launched_entry(pid, facts))
}

/// The names and ids of the entries whose command the process runs, then
/// the id of the entry `launch` started it from and its name, unless an
/// entry running another command shares that name.
fn aliases_with(index: &AliasIndex, facts: &ProcessFacts, launched: Option<String>) -> Vec<String> {
    let mut aliases = facts
        .executable
        .as_ref()
        .map(|actual| {
            let mut aliases = index.names_for(actual);
            aliases.extend(index.ids_for(actual));
            aliases
        })
        .unwrap_or_default();
    if let Some(entry) = launched.and_then(|desktop_id| index.entry(&desktop_id)) {
        let path = crate::system::launch::launch_search::Search::from_env().path_dirs;
        let name_is_unshared = index
            .entries_named(&entry.name)
            .iter()
            .all(|other| other.runs_same_command(entry, &path));
        let recorded = [name_is_unshared.then_some(&entry.name), Some(&entry.id)];
        for alias in recorded.into_iter().flatten() {
            if !aliases
                .iter()
                .any(|known| known.eq_ignore_ascii_case(alias))
            {
                aliases.push(alias.clone());
            }
        }
    }
    aliases
}

/// Exact listed executable name or an alias backed by executable evidence.
pub(crate) fn name_matches(query: &str, name: &str, aliases: &[String]) -> bool {
    std::iter::once(name)
        .chain(aliases.iter().map(String::as_str))
        .any(|candidate| candidate.eq_ignore_ascii_case(query))
}

#[cfg(test)]
#[path = "app_name_tests.rs"]
mod tests;
