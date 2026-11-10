use crate::system::appkit_bridge::workspace_snapshot_json;
use agent_desktop_core::{AdapterError, AppInfo, AppInventory, ErrorCode};
use serde::Deserialize;
use std::time::Instant;
const MAX_SNAPSHOT_BYTES: usize = 1024 * 1024;
const MAX_FIELD_BYTES: usize = 16 * 1024;
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ActivationPolicy {
    Regular,
    Accessory,
    Prohibited,
}

#[derive(Deserialize)]
struct BridgedApplication {
    name: String,
    pid: i32,
    #[serde(default)]
    bundle_id: Option<String>,
    executable: Option<String>,
    launch_time: NullableLaunchTime,
    activation_policy: ActivationPolicy,
}

#[derive(Deserialize)]
struct BridgedWorkspaceSnapshot {
    applications: Vec<BridgedApplication>,
    frontmost_pid: i32,
    frontmost_launch_time: NullableLaunchTime,
    #[serde(default)]
    skipped: Vec<serde_json::Value>,
    /// Set when the bridge read records only for the apps a caller selected.
    #[serde(default)]
    selected_only: bool,
}

#[derive(Deserialize)]
#[serde(transparent)]
struct NullableLaunchTime(serde_json::Value);

impl NullableLaunchTime {
    fn value(&self) -> Option<Option<f64>> {
        match &self.0 {
            serde_json::Value::Null => Some(None),
            serde_json::Value::Number(number) => number.as_f64().map(Some),
            _ => None,
        }
    }
}

pub(crate) fn list_apps_inventory_until(deadline: Instant) -> Result<AppInventory, AdapterError> {
    #[cfg(test)]
    if let Some(inventory) = crate::system::app::app_inventory::adapter_tests::workspace_inventory()
    {
        return Ok(inventory);
    }
    list_apps_with(deadline, workspace_snapshot_json, |_| true, true)
}

pub(crate) fn require_complete(skipped: &[serde_json::Value]) -> Result<(), AdapterError> {
    if skipped.is_empty() {
        return Ok(());
    }
    Err(records::with_skipped(
        inventory_error("AppKit application inventory skipped records"),
        skipped,
    ))
}

pub(super) fn scoped_apps_from(
    snapshot: impl FnOnce() -> Result<Vec<u8>, AdapterError>,
    name: &str,
    bundle_id: Option<&str>,
    deadline: Instant,
) -> Result<AppInventory, AdapterError> {
    list_apps_with(
        deadline,
        snapshot,
        |app| {
            (agent_desktop_core::app_name_matches(&app.name, name)
                || (bundle_id.is_none() && has_alias(&app.bundle_id, &app.executable, name)))
                && bundle_id.is_none_or(|bundle| {
                    app.bundle_id
                        .as_deref()
                        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(bundle))
                })
        },
        false,
    )
}

/// Whether `identifier` is the app's bundle ID or executable name.
fn has_alias(bundle_id: &Option<String>, executable: &Option<String>, identifier: &str) -> bool {
    [bundle_id, executable]
        .into_iter()
        .flatten()
        .any(|alias| alias.eq_ignore_ascii_case(identifier))
}

fn list_apps_with(
    deadline: Instant,
    snapshot: impl FnOnce() -> Result<Vec<u8>, AdapterError>,
    include: impl Fn(&BridgedApplication) -> bool,
    skip_cross_uid: bool,
) -> Result<AppInventory, AdapterError> {
    ensure_before_deadline(deadline)?;
    crate::system::cocoa_runtime::ensure_cocoa_multithreaded()?;
    let bytes = snapshot()?;
    ensure_before_deadline(deadline)?;
    apps_inventory_from_json_with(
        &bytes,
        deadline,
        include,
        skip_cross_uid,
        crate::system::process::process_identity::token_for_pid,
    )
}

#[cfg(test)]
fn apps_from_json(bytes: &[u8], deadline: Instant) -> Result<Vec<AppInfo>, AdapterError> {
    apps_from_json_with(
        bytes,
        deadline,
        |_| true,
        true,
        crate::system::process::process_identity::token_for_pid,
    )
}

#[cfg(test)]
fn apps_from_json_with(
    bytes: &[u8],
    deadline: Instant,
    include: impl Fn(&BridgedApplication) -> bool,
    skip_cross_uid: bool,
    resolve: impl FnMut(i32) -> Result<Option<String>, AdapterError>,
) -> Result<Vec<AppInfo>, AdapterError> {
    Ok(apps_inventory_from_json_with(bytes, deadline, include, skip_cross_uid, resolve)?.apps)
}

fn apps_inventory_from_json_with(
    bytes: &[u8],
    deadline: Instant,
    include: impl Fn(&BridgedApplication) -> bool,
    skip_cross_uid: bool,
    mut resolve: impl FnMut(i32) -> Result<Option<String>, AdapterError>,
) -> Result<AppInventory, AdapterError> {
    let bridged = bridged_snapshot(bytes)?;
    let mut apps = Vec::with_capacity(bridged.applications.len());
    for app in bridged
        .applications
        .into_iter()
        .filter(|app| app.activation_policy != ActivationPolicy::Prohibited)
        .filter(include)
    {
        ensure_before_deadline(deadline)
            .map_err(|error| records::with_skipped(error, &bridged.skipped))?;
        let process_instance = match resolve(app.pid) {
            Ok(Some(instance)) => instance,
            Ok(None) => continue,
            Err(error)
                if skip_cross_uid
                    && crate::system::process::process_apps::is_cross_uid_identity_error(
                        &error,
                    ) =>
            {
                continue;
            }
            Err(error) => return Err(records::with_skipped(error, &bridged.skipped)),
        };
        apps.push(AppInfo {
            name: app.name,
            pid: crate::system::process::process_identity::from_pid_t(app.pid)?,
            bundle_id: app.bundle_id,
            process_instance: Some(process_instance),
            presentation: Some(presentation_of(app.activation_policy)),
        });
    }
    ensure_before_deadline(deadline)
        .map_err(|error| records::with_skipped(error, &bridged.skipped))?;
    Ok(AppInventory {
        apps,
        skipped: bridged.skipped,
    })
}

fn presentation_of(policy: ActivationPolicy) -> agent_desktop_core::AppPresentation {
    match policy {
        ActivationPolicy::Regular => agent_desktop_core::AppPresentation::Foreground,
        ActivationPolicy::Accessory | ActivationPolicy::Prohibited => {
            agent_desktop_core::AppPresentation::Background
        }
    }
}

fn valid_launch_time(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

fn ensure_before_deadline(deadline: Instant) -> Result<(), AdapterError> {
    if Instant::now() >= deadline {
        return Err(AdapterError::timeout("NSWorkspace app inventory timed out"));
    }
    Ok(())
}

fn inventory_error(message: &str) -> AdapterError {
    AdapterError::new(ErrorCode::AppUnresponsive, message)
        .with_suggestion("Retry after macOS finishes updating the running-application inventory")
        .with_details(serde_json::json!({
            "kind": "inventory_source",
            "source": "ns_workspace",
            "complete": false,
            "retryable": true,
        }))
}

#[path = "workspace_apps_owners.rs"]
mod owners;
#[cfg(test)]
use owners::window_owner_snapshot_from_json;
pub(crate) use owners::{
    WindowOwnerSnapshot, focused_owner_until, named_window_owner_snapshot_until,
    window_owner_snapshot_until,
};

#[cfg(test)]
#[path = "workspace_apps_pid_tests.rs"]
mod pid_tests;

#[cfg(test)]
#[path = "workspace_apps_tests.rs"]
mod tests;

#[path = "workspace_apps_records.rs"]
mod records;
use records::bridged_snapshot;

#[cfg(test)]
#[path = "workspace_apps_skip_tests.rs"]
mod skip_tests;
