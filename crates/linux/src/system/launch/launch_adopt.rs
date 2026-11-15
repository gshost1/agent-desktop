use std::collections::HashMap;
use std::ffi::OsStr;

use agent_desktop_core::{Deadline, WindowInfo};

use super::baseline::{Baseline, capture};
use super::launch_owner_query::OwnerQuery;
use super::launch_step::Step;
use crate::system::SharedBus;
use crate::system::process::executable_identity::ExecutableIdentity;
use crate::system::process::running_app::Running;
use crate::system::window::window_ops::windows_of_process;

#[path = "launch_adopt_bus.rs"]
mod bus;

/// The running instance a cleanly exited launcher handed the launch to when
/// no resident gained a window, as when a running application only shows a
/// different page of the window it has. `serving` caches, per pid, whether
/// the instance receives forwarded launches.
pub(super) fn adopt(
    bus: &SharedBus,
    query: &OwnerQuery<'_>,
    apps: &[Running],
    serving: &mut HashMap<u32, bool>,
) -> Step {
    choose(
        query.before,
        query.identity,
        apps,
        |owner| capture(bus, owner, query.deadline).is_ok(),
        |owner| {
            *serving.entry(owner.pid).or_insert_with(|| {
                bus::serves_launches(owner.pid, query.desktop_id, query.deadline)
            })
        },
    )
}

/// The one instance of the requested executable that ran before the spawn,
/// whose windows were read then and are read again now, whose first operand
/// matches the request's, and which `serves` forwarded launches. Anything
/// else is not proof of a handoff.
fn choose(
    before: &Baseline,
    requested: &Option<ExecutableIdentity>,
    apps: &[Running],
    read: impl FnOnce(&Running) -> bool,
    serves: impl FnOnce(&Running) -> bool,
) -> Step {
    let [(resident, _)] = before.residents.as_slice() else {
        return Step::Wait;
    };
    if before.instances != 1 || !same_operand(resident, requested) {
        return Step::Wait;
    }
    match apps
        .iter()
        .find(|app| app.identity() == resident.identity())
    {
        Some(live) if read(live) && serves(live) => Step::Owner(live.clone()),
        _ => Step::Wait,
    }
}

fn first_operand(identity: Option<&ExecutableIdentity>) -> Option<&OsStr> {
    identity?
        .argv
        .as_ref()?
        .iter()
        .skip(1)
        .map(|argument| argument.as_os_str())
        .find(|argument| !argument.as_encoded_bytes().starts_with(b"-"))
}

/// A resident started with an operand, such as the script an interpreter
/// runs, takes only a request with the same first operand.
fn same_operand(resident: &Running, requested: &Option<ExecutableIdentity>) -> bool {
    match first_operand(resident.executable.as_ref()) {
        None => true,
        operand => operand == first_operand(requested.as_ref()),
    }
}

/// The window of an adopted instance: its only window, or none when it shows
/// several, because nothing tells which of them the launch changed.
pub(super) fn adopted_window(
    bus: &SharedBus,
    owner: &Running,
    deadline: Deadline,
) -> Option<WindowInfo> {
    let (windows, _) = windows_of_process(bus, owner.pid, deadline).ok()?;
    only_window(owner, windows)
}

fn only_window(owner: &Running, windows: Vec<WindowInfo>) -> Option<WindowInfo> {
    let mut own = windows.into_iter().filter(|window| {
        window.pid.get() == owner.pid
            && window.process_instance.as_deref() == Some(owner.instance.as_str())
    });
    match (own.next(), own.next()) {
        (Some(window), None) => Some(window),
        _ => None,
    }
}

/// The note on an adopted launch, ahead of any note the result already has.
pub(super) fn adoption_note(pid: u32, earlier: Option<String>) -> String {
    let note = format!(
        "The launcher exited cleanly and no new window opened, so the running instance (pid {pid}) is reported as the owner. The window, if any, was already open, and the effect of the launch on it was not verified."
    );
    match earlier {
        Some(earlier) => format!("{note} {earlier}"),
        None => note,
    }
}

#[cfg(test)]
#[path = "launch_adopt_tests.rs"]
mod tests;
