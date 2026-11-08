use crate::system::{window::cg_window::WindowRecord, window::window_ax_state::WindowAxState};
use agent_desktop_core::{AdapterError, ErrorCode, WindowInfo};
use rustc_hash::{FxHashMap, FxHashSet};
use std::time::Instant;

impl OwnerSnapshotView for crate::system::app::workspace_apps::WindowOwnerSnapshot {
    fn eligible_pids(&self) -> FxHashSet<i32> {
        crate::system::app::workspace_apps::WindowOwnerSnapshot::eligible_pids(self)
    }

    fn generation(&self, pid: i32) -> Option<OwnerGeneration> {
        self.owner(pid).map(|owner| match owner.launch_time {
            Some(launch_time) => OwnerGeneration::LaunchTime(launch_time),
            None => OwnerGeneration::LiveProcess,
        })
    }

    fn frontmost_pid(&self) -> Option<i32> {
        self.frontmost().map(|owner| owner.pid)
    }

    fn same_generation(&self, other: &Self) -> bool {
        crate::system::app::workspace_apps::WindowOwnerSnapshot::same_generation(self, other)
    }
}

pub(in crate::system) trait OwnerSnapshotView {
    fn eligible_pids(&self) -> FxHashSet<i32>;
    fn generation(&self, pid: i32) -> Option<OwnerGeneration>;
    fn frontmost_pid(&self) -> Option<i32>;
    fn same_generation(&self, other: &Self) -> bool;
}

#[derive(Clone, Copy, PartialEq)]
pub(in crate::system) enum OwnerGeneration {
    LaunchTime(f64),
    LiveProcess,
}

pub(crate) fn list_windows_until(
    focused_only: bool,
    deadline: Instant,
) -> Result<Vec<WindowInfo>, AdapterError> {
    stabilize_global_with(deadline, || capture_global_once(focused_only, deadline))
}

fn capture_global_once(
    focused_only: bool,
    deadline: Instant,
) -> Result<Vec<WindowInfo>, AdapterError> {
    capture_global_with(
        focused_only,
        || crate::system::app::workspace_apps::window_owner_snapshot_until(deadline),
        |eligible_pids| {
            crate::system::window::cg_window::window_records_until(
                deadline,
                crate::system::window::cg_window::WindowRecordScope::Pids(eligible_pids),
            )
        },
        |pid| crate::system::window::window_ax_state::read_frontmost_until(pid, deadline),
        |records, pids| {
            crate::system::window::window_title::probe_owners_until(records, pids, deadline)
        },
    )
}

/// The owner probe runs once, after the owner snapshots agree, so the churn
/// window between them holds only the CoreGraphics read and the frontmost
/// focus read, and a churn retry never repeats the probe.
fn capture_global_with<S: OwnerSnapshotView>(
    focused_only: bool,
    capture_owners: impl FnMut() -> Result<S, AdapterError>,
    capture_records: impl FnMut(&FxHashSet<i32>) -> Result<Vec<WindowRecord>, AdapterError>,
    mut read_frontmost: impl FnMut(i32) -> Result<WindowAxState, AdapterError>,
    probe_owners: impl FnOnce(&[WindowRecord], &[i32]) -> FxHashMap<i32, WindowAxState>,
) -> Result<Vec<WindowInfo>, AdapterError> {
    let capture = capture_stable_with(
        capture_owners,
        OwnerSnapshotView::eligible_pids,
        capture_records,
        |records, owners| capture_frontmost_focus(records, owners, &mut read_frontmost),
    )?;
    let states = probe_owners(&capture.records, &capture.probe_pids(focused_only));
    finish_global_windows(capture, &states, focused_only)
}

pub(in crate::system) fn capture_stable_with<S: OwnerSnapshotView, T>(
    capture_owners: impl FnMut() -> Result<S, AdapterError>,
    select_pids: impl FnMut(&S) -> FxHashSet<i32>,
    capture_records: impl FnMut(&FxHashSet<i32>) -> Result<Vec<WindowRecord>, AdapterError>,
    assemble: impl FnMut(Vec<WindowRecord>, &S) -> Result<T, AdapterError>,
) -> Result<T, AdapterError> {
    capture_validated_with(
        capture_owners,
        select_pids,
        capture_records,
        assemble,
        |before, after, _, records| validate_snapshot_pair(before, after, records),
    )
}

pub(in crate::system) fn capture_validated_with<S: OwnerSnapshotView, T>(
    mut capture_owners: impl FnMut() -> Result<S, AdapterError>,
    mut select_pids: impl FnMut(&S) -> FxHashSet<i32>,
    mut capture_records: impl FnMut(&FxHashSet<i32>) -> Result<Vec<WindowRecord>, AdapterError>,
    mut assemble: impl FnMut(Vec<WindowRecord>, &S) -> Result<T, AdapterError>,
    mut validate: impl FnMut(&S, &S, &FxHashSet<i32>, &[WindowRecord]) -> Result<(), AdapterError>,
) -> Result<T, AdapterError> {
    let before = capture_owners()?;
    let eligible_pids = select_pids(&before);
    let records = capture_records(&eligible_pids)?;
    let assembled = assemble(records.clone(), &before);
    let after = capture_owners()?;
    validate(&before, &after, &eligible_pids, &records)?;
    assembled
}

pub(in crate::system) fn validate_snapshot_pair<S: OwnerSnapshotView>(
    before: &S,
    after: &S,
    records: &[WindowRecord],
) -> Result<(), AdapterError> {
    if !before.same_generation(after) {
        return Err(owner_churn_error(None, "appkit_snapshot_changed"));
    }
    validate_record_generations(records, after)
}

pub(in crate::system) fn validate_scoped_snapshot_pair<S: OwnerSnapshotView>(
    before: &S,
    after: &S,
    before_pids: &FxHashSet<i32>,
    after_pids: &FxHashSet<i32>,
    records: &[WindowRecord],
) -> Result<(), AdapterError> {
    if before_pids != after_pids
        || before_pids
            .iter()
            .any(|pid| before.generation(*pid) != after.generation(*pid))
    {
        return Err(owner_churn_error(None, "selected_appkit_snapshot_changed"));
    }
    validate_record_generations(records, after)
}

pub(in crate::system) fn stabilize_global_with(
    deadline: Instant,
    mut capture: impl FnMut() -> Result<Vec<WindowInfo>, AdapterError>,
) -> Result<Vec<WindowInfo>, AdapterError> {
    let mut attempts = 0_u64;
    let mut last_error = None;
    loop {
        if Instant::now() >= deadline {
            return Err(global_timeout(attempts, last_error.as_ref()));
        }
        attempts += 1;
        match capture() {
            Ok(windows) => return Ok(windows),
            Err(error) if retryable_global_error(&error) => {
                last_error = Some(error);
            }
            Err(error) if error.code == ErrorCode::Timeout && last_error.is_some() => {
                return Err(global_timeout(attempts, last_error.as_ref()));
            }
            Err(error) => return Err(error),
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if !remaining.is_zero() {
            std::thread::sleep(remaining.min(std::time::Duration::from_millis(5)));
        }
    }
}

fn validate_record_generations<S: OwnerSnapshotView>(
    records: &[WindowRecord],
    owners: &S,
) -> Result<(), AdapterError> {
    for (pid, instance) in unique_process_instances(records)? {
        let generation = owners
            .generation(pid)
            .ok_or_else(|| owner_churn_error(Some(pid), "owner_not_eligible"))?;
        let matches = match generation {
            OwnerGeneration::LaunchTime(launch_time) => {
                crate::system::process::process_identity::instance_matches_launch_time(
                    pid,
                    instance,
                    launch_time,
                )?
            }
            OwnerGeneration::LiveProcess => {
                crate::system::process::process_identity::matches_instance(pid, instance)?
            }
        };
        if !matches {
            return Err(owner_churn_error(Some(pid), "process_generation_changed"));
        }
    }
    Ok(())
}

fn unique_process_instances(records: &[WindowRecord]) -> Result<Vec<(i32, &str)>, AdapterError> {
    let mut instances = FxHashMap::default();
    for record in records {
        let instance = record
            .process_instance
            .as_deref()
            .filter(|instance| !instance.is_empty())
            .ok_or_else(|| owner_churn_error(Some(record.pid), "process_instance_missing"))?;
        match instances.entry(record.pid) {
            std::collections::hash_map::Entry::Occupied(entry) => {
                if *entry.get() != instance {
                    return Err(owner_churn_error(
                        Some(record.pid),
                        "process_instance_conflict",
                    ));
                }
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(instance);
            }
        }
    }
    let mut instances = instances.into_iter().collect::<Vec<_>>();
    instances.sort_unstable_by_key(|(pid, _)| *pid);
    Ok(instances)
}

fn owner_churn_error(pid: Option<i32>, phase: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Window owner identity changed during global inventory",
    )
    .with_details(serde_json::json!({
        "kind": "global_window_owner_churn",
        "pid": pid,
        "phase": phase,
        "complete": false,
        "retryable": true,
    }))
}

fn retryable_global_error(error: &AdapterError) -> bool {
    error.code == ErrorCode::AppUnresponsive && error.is_explicitly_retryable()
}

#[path = "window_inventory_global_assemble.rs"]
mod assemble;
use assemble::{capture_frontmost_focus, finish_global_windows};

#[path = "window_inventory_global_timeout.rs"]
mod timeout;
use timeout::global_timeout;

#[cfg(test)]
#[path = "window_inventory_global_timeout_tests.rs"]
mod timeout_tests;

#[cfg(test)]
#[path = "window_inventory_global_tests.rs"]
mod tests;
