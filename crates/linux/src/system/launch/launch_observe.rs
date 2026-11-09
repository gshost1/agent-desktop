use crate::system::launch::launch_sample::Sample;
use crate::system::launch::launch_step::Step;
use std::collections::HashSet;
use std::os::unix::process::ExitStatusExt;
use std::process::{Child, ExitStatus};
use std::time::Duration;

use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode, ProcessId};

use super::launch_attach::matches;
use super::launch_handoff;
use super::launch_owner_query::OwnerQuery;
use crate::system::SharedBus;
use crate::system::process::running_app::Running;
use crate::system::registry::Registry;

pub(in crate::system) const POLL_INTERVAL: Duration = Duration::from_millis(50);
const POLL_LIMIT: Duration = Duration::from_millis(250);

fn pick(mut found: Vec<&Running>) -> Option<Step> {
    match found.len() {
        0 => None,
        1 => Some(Step::Owner(found.remove(0).clone())),
        _ => Some(Step::Ambiguous(found.iter().map(|app| app.pid).collect())),
    }
}

/// The next move of a launch. The owner is the child while it runs and shows
/// on the registry, else an application matching the executable identity that was
/// absent before the spawn, else (when the launcher exited cleanly after
/// forwarding) the one that was already running. A launcher killed by a signal
/// or exiting with a failure and no owner is a failed launch, and a clean
/// exit with no owner is one too at the launch deadline.
pub(crate) fn step(sample: &Sample<'_>) -> Step {
    let candidates: Vec<&Running> = sample
        .apps
        .iter()
        .filter(|app| matches(sample.identity, app))
        .collect();
    let new: Vec<&Running> = candidates
        .iter()
        .copied()
        .filter(|app| !sample.before.contains(&app.identity()))
        .collect();
    let own = sample.apps.iter().find(|app| app.pid == sample.child_pid);
    match sample.child {
        None => own
            .map(|app| Step::Owner(app.clone()))
            .or_else(|| pick(new))
            .unwrap_or(Step::Wait),
        Some(status) if status.code().is_none() => Step::Failed(status),
        Some(status) => {
            let code = status.code();
            pick(new)
                .or_else(|| {
                    if code == Some(0) {
                        pick(candidates)
                    } else {
                        None
                    }
                })
                .unwrap_or(if code.is_some_and(|code| code != 0) || sample.settled {
                    Step::Failed(status)
                } else {
                    Step::Wait
                })
        }
    }
}

/// The registry as one record per process, with the facts launching reads.
pub(crate) fn running_of(registry: &Registry) -> Result<Vec<Running>, AdapterError> {
    if registry
        .skipped
        .iter()
        .any(|record| record["confirmed_gone"] != true)
    {
        return Err(
            AdapterError::timeout("The running process inventory could not be fully read")
                .with_details(serde_json::json!({ "skipped": registry.skipped })),
        );
    }
    let mut seen = HashSet::new();
    Ok(registry
        .apps
        .iter()
        .filter(|app| seen.insert((app.root.pid(), app.facts.instance.clone())))
        .map(|app| Running {
            executable: app.facts.executable.clone(),
            pid: app.root.pid(),
            instance: app.facts.instance.clone(),
            name: app.name.clone(),
        })
        .collect())
}

/// A spawned process that is reaped when this guard goes, so an application
/// that outlives a long-running host never becomes a zombie.
pub(crate) struct Spawned {
    child: Option<Child>,
    pub(crate) pid: u32,
    identifier: String,
}

impl Spawned {
    pub(crate) fn new(child: Child, identifier: &str) -> Self {
        Self {
            pid: child.id(),
            child: Some(child),
            identifier: identifier.to_owned(),
        }
    }

    fn failure(&self, status: ExitStatus) -> AdapterError {
        failed(status, &self.identifier)
    }

    fn state(&mut self) -> Option<ExitStatus> {
        self.child
            .as_mut()
            .and_then(|child| child.try_wait().ok().flatten())
    }
}

impl Drop for Spawned {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take()
            && matches!(child.try_wait(), Ok(None))
        {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
    }
}

fn failed(status: ExitStatus, identifier: &str) -> AdapterError {
    let (what, detail) = match status.code() {
        Some(code) => (
            if code == 0 {
                format!(
                    "'{identifier}' exited with status 0 but no running instance of the executable opened a window"
                )
            } else {
                format!(
                    "'{identifier}' exited with status {code} without a verified application owner"
                )
            },
            serde_json::json!({ "kind": "exited", "status": code }),
        ),
        None => {
            let signal = status.signal().unwrap_or_default();
            (
                format!("'{identifier}' was killed by signal {signal} before it started"),
                serde_json::json!({ "kind": "crashed", "signal": signal }),
            )
        }
    };
    AdapterError::new(ErrorCode::ActionFailed, what)
        .with_details(detail)
        .with_suggestion(
            "The launcher may have run and handed off to an application this tool cannot match; run 'list-apps' to see what is running.",
        )
        .with_disposition(DeliverySemantics::delivered_unverified())
}

pub(in crate::system) fn pids_of(pids: &[u32]) -> Vec<ProcessId> {
    pids.iter().copied().map(ProcessId::new).collect()
}

fn ambiguous(pids: &[u32], identifier: &str) -> AdapterError {
    AdapterError::ambiguous_process_target(
        format!("Several new applications match '{identifier}'"),
        &pids_of(pids),
    )
    .with_disposition(DeliverySemantics::delivered_unverified())
}

fn after_spawn(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::delivered_unverified())
}

/// Polls the registry until the launch has an owner, the launcher failed, or
/// the deadline. A child that is alive with no registry entry at the deadline,
/// or after the query's not-on-bus grace, is reported as not on the bus: the
/// pid is the spawned process's.
pub(super) fn find_owner(
    bus: &SharedBus,
    spawned: &mut Spawned,
    query: &OwnerQuery<'_>,
) -> Result<Option<Running>, AdapterError> {
    let deadline = query.deadline;
    poll_owner(
        spawned,
        query,
        || match crate::system::registry::read_processes(bus, deadline) {
            Ok(registry) => running_of(&registry).map(Some),
            Err(error) if error.code == ErrorCode::Timeout => Ok(None),
            Err(error) => Err(error),
        },
        |apps| launch_handoff::observe(bus, query.before, apps, deadline),
    )
}

/// The owner search over a registry reader. `read` answers `None` when the
/// registry could not be read in time, which is not evidence that the
/// spawned process is off the bus. `handoff` looks for a resident that
/// received the launch from a launcher that exited cleanly.
fn poll_owner(
    spawned: &mut Spawned,
    query: &OwnerQuery<'_>,
    mut read: impl FnMut() -> Result<Option<Vec<Running>>, AdapterError>,
    mut handoff: impl FnMut(&[Running]) -> Step,
) -> Result<Option<Running>, AdapterError> {
    let deadline = query.deadline;
    let mut interval = POLL_INTERVAL;
    loop {
        let child = spawned.state();
        let expired = deadline.is_expired();
        let listed = read().map_err(after_spawn)?;
        let registry_read = listed.is_some();
        let apps = listed.unwrap_or_default();
        let sample = Sample {
            child_pid: spawned.pid,
            child,
            apps: &apps,
            before: &query.before.processes,
            identity: query.identity,
            settled: expired,
        };
        let mut next = step(&sample);
        if let Some(status) = child
            && status.success()
            && matches!(next, Step::Wait | Step::Failed(_))
        {
            next = handoff(&apps);
            if next == Step::Wait && expired {
                next = Step::Failed(status);
            }
        }
        let not_on_bus = child.is_none() && registry_read && query.not_on_bus.is_expired();
        match next {
            Step::Owner(owner) => return Ok(Some(owner)),
            Step::Ambiguous(pids) => return Err(ambiguous(&pids, &spawned.identifier)),
            Step::Failed(status) => return Err(spawned.failure(status)),
            Step::Wait if expired || not_on_bus => return Ok(None),
            Step::Wait => {}
        }
        std::thread::sleep(interval.min(deadline.remaining()));
        interval = (interval * 3 / 2).min(POLL_LIMIT);
    }
}

#[cfg(test)]
#[path = "launch_observe_tests.rs"]
mod tests;
