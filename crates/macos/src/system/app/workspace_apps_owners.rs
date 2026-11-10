//! The window owners an inventory read joins windows to: every app that can
//! own a window, with its launch identity and which one is frontmost.

use super::{
    ActivationPolicy, ensure_before_deadline, has_alias, inventory_error, records,
    records::bridged_snapshot, require_complete, valid_launch_time,
};
use agent_desktop_core::AdapterError;
use std::time::Instant;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WindowOwner {
    pub(crate) pid: i32,
    pub(crate) name: String,
    pub(crate) bundle_id: Option<String>,
    pub(crate) executable: Option<String>,
    pub(crate) launch_time: Option<f64>,
    pub(crate) activation_policy: ActivationPolicy,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WindowOwnerSnapshot {
    pub(super) owners: Vec<WindowOwner>,
    pub(super) frontmost_pid: Option<i32>,
    pub(super) skipped: Vec<serde_json::Value>,
}

impl WindowOwnerSnapshot {
    pub(crate) fn eligible_pids(&self) -> rustc_hash::FxHashSet<i32> {
        self.owners.iter().map(|owner| owner.pid).collect()
    }

    /// Pids of the apps `identifier` names. A bundle ID or executable name counts
    /// only when no app has that display name, so an alias never merges two apps.
    pub(crate) fn matching_pids(&self, identifier: &str) -> rustc_hash::FxHashSet<i32> {
        let named =
            |owner: &WindowOwner| agent_desktop_core::app_name_matches(&owner.name, identifier);
        let by_alias = !self.owners.iter().any(named);
        self.owners
            .iter()
            .filter(|owner| {
                named(owner)
                    || (by_alias && has_alias(&owner.bundle_id, &owner.executable, identifier))
            })
            .map(|owner| owner.pid)
            .collect()
    }

    pub(crate) fn require_match(&self, identifier: &str) -> Result<(), AdapterError> {
        if self.matching_pids(identifier).is_empty() {
            require_complete(&self.skipped)?;
        }
        Ok(())
    }

    pub(crate) fn owner(&self, pid: i32) -> Option<&WindowOwner> {
        self.owners.iter().find(|owner| owner.pid == pid)
    }

    pub(crate) fn frontmost(&self) -> Option<&WindowOwner> {
        self.frontmost_pid.and_then(|pid| self.owner(pid))
    }

    pub(crate) fn same_generation(&self, other: &Self) -> bool {
        self == other
    }
}

pub(crate) fn focused_owner_until(deadline: Instant) -> Result<Option<WindowOwner>, AdapterError> {
    ensure_before_deadline(deadline)?;
    crate::system::cocoa_runtime::ensure_cocoa_multithreaded()?;
    let bytes = crate::system::appkit_bridge::focused_snapshot_json()?;
    ensure_before_deadline(deadline)?;
    let snapshot = window_owner_snapshot_from_json(&bytes, deadline)?;
    require_complete(&snapshot.skipped)?;
    Ok(snapshot.frontmost().cloned())
}

pub(crate) fn window_owner_snapshot_until(
    deadline: Instant,
) -> Result<WindowOwnerSnapshot, AdapterError> {
    ensure_before_deadline(deadline)?;
    crate::system::cocoa_runtime::ensure_cocoa_multithreaded()?;
    let bytes = crate::system::appkit_bridge::workspace_snapshot_json()?;
    ensure_before_deadline(deadline)?;
    window_owner_snapshot_from_json(&bytes, deadline)
}

pub(super) fn window_owner_snapshot_from_json(
    bytes: &[u8],
    deadline: Instant,
) -> Result<WindowOwnerSnapshot, AdapterError> {
    let bridged = bridged_snapshot(bytes)?;
    let mut owners = Vec::with_capacity(bridged.applications.len());
    for app in bridged
        .applications
        .into_iter()
        .filter(|app| app.activation_policy != ActivationPolicy::Prohibited)
    {
        ensure_before_deadline(deadline)
            .map_err(|error| records::with_skipped(error, &bridged.skipped))?;
        let Some(launch_time) = app.launch_time.value() else {
            return Err(inventory_error(
                "AppKit returned invalid window-owner identity",
            ));
        };
        owners.push(WindowOwner {
            pid: app.pid,
            name: app.name,
            bundle_id: app.bundle_id,
            executable: app.executable,
            launch_time,
            activation_policy: app.activation_policy,
        });
    }
    owners.sort_unstable_by_key(|owner| owner.pid);
    let Some(frontmost_launch_time) = bridged.frontmost_launch_time.value() else {
        return Err(inventory_error(
            "AppKit returned incomplete frontmost-application identity",
        ));
    };
    let frontmost_pid = if bridged
        .skipped
        .iter()
        .any(|record| record["pid"].as_i64() == Some(i64::from(bridged.frontmost_pid)))
        && !owners
            .iter()
            .any(|owner| owner.pid == bridged.frontmost_pid)
    {
        None
    } else {
        validate_frontmost(&mut owners, bridged.frontmost_pid, frontmost_launch_time)
            .map_err(|error| records::with_skipped(error, &bridged.skipped))?
    };
    ensure_before_deadline(deadline)
        .map_err(|error| records::with_skipped(error, &bridged.skipped))?;
    Ok(WindowOwnerSnapshot {
        owners,
        frontmost_pid,
        skipped: bridged.skipped,
    })
}

fn validate_frontmost(
    owners: &mut [WindowOwner],
    pid: i32,
    launch_time: Option<f64>,
) -> Result<Option<i32>, AdapterError> {
    if pid == 0 && launch_time.is_none() {
        return Ok(None);
    }
    if pid <= 0 || launch_time.is_some_and(|value| !valid_launch_time(value)) {
        return Err(inventory_error(
            "AppKit returned incomplete frontmost-application identity",
        ));
    }
    let Some(owner) = owners.iter_mut().find(|owner| owner.pid == pid) else {
        return Err(inventory_error(
            "AppKit frontmost application did not exactly match an eligible window owner",
        ));
    };
    match (owner.launch_time, launch_time) {
        (Some(owner_launch_time), Some(frontmost_launch_time))
            if owner_launch_time.to_bits() != frontmost_launch_time.to_bits() =>
        {
            return Err(inventory_error(
                "AppKit frontmost application did not exactly match an eligible window owner",
            ));
        }
        (None, Some(frontmost_launch_time)) => {
            owner.launch_time = Some(frontmost_launch_time);
        }
        _ => {}
    }
    Ok(Some(pid))
}
