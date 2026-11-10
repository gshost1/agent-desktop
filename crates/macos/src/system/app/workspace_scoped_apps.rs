use agent_desktop_core::{AdapterError, AppInfo, AppInventory};
use std::time::Instant;

use super::workspace_apps::scoped_apps_from;
use crate::system::appkit_bridge::{named_workspace_snapshot_json, workspace_snapshot_json};

/// Apps named `name`, optionally pinned to `bundle_id`. Without one, a bundle
/// ID or executable name matches only when no app has that display name.
///
/// AppKit fetches each app's identity fields from LaunchServices on every read, so the read takes
/// full records only for the apps whose display name matches, and every record when none does.
/// When the named apps it returned all drop out, every app is read again, so an alias still
/// matches exactly when it did before.
pub(crate) fn list_apps_scoped_until(
    name: &str,
    bundle_id: Option<&str>,
    deadline: Instant,
) -> Result<AppInventory, AdapterError> {
    let reads = WorkspaceReads {
        named: named_workspace_snapshot_json,
        every: workspace_snapshot_json,
    };
    list_apps_scoped_from(&reads, name, bundle_id, deadline)
}

/// The two AppKit reads a scoped lookup chooses between: records only for
/// the selected display names, or every record.
struct WorkspaceReads<N, E> {
    named: N,
    every: E,
}

fn list_apps_scoped_from<N, E>(
    reads: &WorkspaceReads<N, E>,
    name: &str,
    bundle_id: Option<&str>,
    deadline: Instant,
) -> Result<AppInventory, AdapterError>
where
    N: Fn(&dyn Fn(&str) -> bool) -> Result<Vec<u8>, AdapterError>,
    E: Fn() -> Result<Vec<u8>, AdapterError>,
{
    let selects = |candidate: &str| agent_desktop_core::app_name_matches(candidate, name);
    scoped_inventory(name, |named_only| {
        if !named_only {
            let inventory = scoped_apps_from(&reads.every, name, bundle_id, deadline)?;
            return Ok((inventory, true));
        }
        let mut every_app = true;
        let read = || {
            let bytes = (reads.named)(&selects)?;
            every_app = !selected_only(&bytes);
            Ok(bytes)
        };
        let inventory = scoped_apps_from(read, name, bundle_id, deadline)?;
        Ok((inventory, every_app))
    })
}

fn selected_only(bytes: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(bytes)
        .ok()
        .and_then(|snapshot| snapshot.get("selected_only")?.as_bool())
        .unwrap_or(false)
}

fn scoped_inventory(
    name: &str,
    mut capture: impl FnMut(bool) -> Result<(AppInventory, bool), AdapterError>,
) -> Result<AppInventory, AdapterError> {
    let named = |app: &AppInfo| agent_desktop_core::app_name_matches(&app.name, name);
    let (mut inventory, every_app) = capture(true)?;
    if !every_app && !inventory.apps.iter().any(named) {
        inventory = capture(false)?.0;
    }
    if inventory.apps.iter().any(named) {
        inventory.apps.retain(named);
    }
    Ok(inventory)
}

#[cfg(test)]
#[path = "workspace_scoped_apps_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "workspace_scoped_reads_tests.rs"]
mod reads_tests;
