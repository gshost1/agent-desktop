use std::collections::HashSet;

use agent_desktop_core::Deadline;

use super::baseline::{Baseline, capture, same_executable};
use super::launch_step::Step;
use crate::system::SharedBus;
use crate::system::process::running_app::Running;

/// The resident a cleanly exited launcher handed the launch to: the one that
/// gained a window. Once the handoff has `settled` with no new window, as when
/// a running application only shows a different page of its window, the
/// handoff went to the one resident still running, if there is exactly one.
fn select_owner(
    before: &[(Running, HashSet<String>)],
    after: &[(Running, HashSet<String>)],
    settled: bool,
) -> Step {
    let mut owners = after.iter().filter_map(|(owner, windows)| {
        before.iter().find_map(|(resident, known)| {
            (resident.identity() == owner.identity()
                && windows.iter().any(|id| !known.contains(id)))
            .then_some(owner)
        })
    });
    match (owners.next(), owners.next()) {
        (None, _) if settled => sole_resident(before, after),
        (None, _) => Step::Wait,
        (Some(owner), None) => Step::Owner(owner.clone()),
        (Some(first), Some(second)) => Step::Ambiguous(
            [first.pid, second.pid]
                .into_iter()
                .chain(owners.map(|owner| owner.pid))
                .collect(),
        ),
    }
}

fn sole_resident(
    before: &[(Running, HashSet<String>)],
    after: &[(Running, HashSet<String>)],
) -> Step {
    let mut still_running = after.iter().filter(|(owner, _)| {
        before
            .iter()
            .any(|(resident, _)| resident.identity() == owner.identity())
    });
    match (still_running.next(), still_running.next()) {
        (Some((owner, _)), None) => Step::Owner(owner.clone()),
        _ => Step::Wait,
    }
}

pub(super) fn observe(
    bus: &SharedBus,
    baseline: &Baseline,
    apps: &[Running],
    deadline: Deadline,
    settled: bool,
) -> Step {
    let after: Vec<_> = apps
        .iter()
        .filter(|app| {
            baseline.residents.iter().any(|(resident, _)| {
                resident.identity() == app.identity()
                    && same_executable(resident.executable.as_ref(), app)
            })
        })
        .filter_map(|owner| {
            capture(bus, owner, deadline)
                .ok()
                .map(|windows| (owner.clone(), windows))
        })
        .collect();
    select_owner(&baseline.residents, &after, settled)
}

#[cfg(test)]
#[path = "launch_handoff_tests.rs"]
mod tests;
