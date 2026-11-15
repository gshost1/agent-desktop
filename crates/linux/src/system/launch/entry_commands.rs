//! The other commands a desktop entry starts its application with. The entry's
//! own `Exec` is not the only one: each action it lists runs its own `Exec`
//! (a "New Window" action that runs `editor --new-window`), and an entry with
//! `DBusActivatable=true` is started by the session bus through the service
//! file named after its id, whose `Exec` the bus runs (`editor
//! --gapplication-service`). A process started either way runs one of these
//! commands, not the entry's `Exec`.

use std::path::{Path, PathBuf};

use super::desktop_entry::{GROUP, keys_of, unescape};

const SERVICE_GROUP: &str = "[D-BUS Service]";
const SERVICES: &str = "dbus-1/services";
const DESKTOP_SUFFIX: &str = ".desktop";

fn value<'a>(pairs: &[(&'a str, &'a str)], key: &str) -> Option<String> {
    pairs
        .iter()
        .find(|(candidate, _)| *candidate == key)
        .map(|(_, value)| unescape(value))
        .filter(|value| !value.trim().is_empty())
}

/// The `Exec` of each action the entry's `Actions` key lists, in that order.
/// An action with no group or no `Exec` adds nothing.
pub(in crate::system) fn action_execs(text: &str) -> Vec<String> {
    let entry = keys_of(text, GROUP);
    let Some(actions) = value(&entry, "Actions") else {
        return Vec::new();
    };
    actions
        .split(';')
        .map(str::trim)
        .filter(|action| !action.is_empty())
        .filter_map(|action| {
            value(
                &keys_of(text, &format!("[Desktop Action {action}]")),
                "Exec",
            )
        })
        .collect()
}

/// The bus name an activatable entry owns: its id without `.desktop`, when
/// that is a name no path separator or NUL could turn into another file.
fn bus_name(id: &str) -> Option<&str> {
    let name = id.strip_suffix(DESKTOP_SUFFIX).unwrap_or(id);
    (!name.is_empty() && !name.contains(['/', '\0']) && name != "." && name != "..").then_some(name)
}

/// The `Exec` of the session service that starts an entry marked
/// `DBusActivatable=true`: the first `<name>.service` beside the entry
/// directories (`<data dir>/dbus-1/services`) whose `Name` is the entry's bus
/// name. `applications_dirs` are the `<data dir>/applications` directories the
/// entries were read from.
pub(in crate::system) fn activation_exec(
    id: &str,
    text: &str,
    applications_dirs: &[PathBuf],
) -> Option<String> {
    let activatable =
        value(&keys_of(text, GROUP), "DBusActivatable").is_some_and(|flag| flag == "true");
    if !activatable {
        return None;
    }
    let name = bus_name(id)?;
    applications_dirs
        .iter()
        .filter_map(|dir| dir.parent())
        .map(|data| data.join(SERVICES).join(format!("{name}.service")))
        .find_map(|file| service_exec(&file, name))
}

fn service_exec(file: &Path, name: &str) -> Option<String> {
    let text = std::fs::read_to_string(file).ok()?;
    let pairs = keys_of(&text, SERVICE_GROUP);
    (value(&pairs, "Name").as_deref() == Some(name))
        .then(|| value(&pairs, "Exec"))
        .flatten()
}

/// Every command other than its `Exec` the entry starts its application with:
/// its actions' commands, then its bus activation command.
pub(in crate::system) fn other_execs(
    id: &str,
    text: &str,
    applications_dirs: &[PathBuf],
) -> Vec<String> {
    let mut execs = action_execs(text);
    execs.extend(activation_exec(id, text, applications_dirs));
    execs
}

#[cfg(test)]
#[path = "entry_commands_tests.rs"]
mod tests;
