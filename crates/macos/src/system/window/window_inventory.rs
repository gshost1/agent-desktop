use agent_desktop_core::{AdapterError, WindowFilter, WindowInfo};
use std::time::Instant;

use crate::system::window::cg_window;
use crate::system::window::window_inventory_global::{OwnerGeneration, OwnerSnapshotView};

pub(crate) fn list_windows_until(
    filter: &WindowFilter,
    deadline: Instant,
) -> Result<Vec<WindowInfo>, AdapterError> {
    let Some(app_filter) = filter.app.as_deref().filter(|name| !name.is_empty()) else {
        return crate::system::window::window_inventory_global::list_windows_until(
            filter.focused_only,
            deadline,
        );
    };
    crate::system::window::window_inventory_global::stabilize_global_with(deadline, || {
        let owners = crate::system::app::workspace_apps::named_window_owner_snapshot_until(
            app_filter, deadline,
        )?;
        owners.require_match(app_filter)?;
        let pids = owners.matching_pids(app_filter);
        require_running_app(&pids, app_filter)?;
        let records =
            cg_window::window_records_until(deadline, cg_window::WindowRecordScope::Pids(&pids))?;
        let windows = windows_from_records_with_focus(
            records.clone(),
            filter.focused_only,
            |pid| crate::system::window::window_ax_state::read_until(pid, deadline),
            crate::system::process::process_identity::matches_instance,
        );
        validate_scoped_owners(
            ScopedRead {
                owners: &owners,
                selected: &pids,
                records: &records,
            },
            owner_still_running,
            || {
                let after = crate::system::app::workspace_apps::named_window_owner_snapshot_until(
                    app_filter, deadline,
                )?;
                let after_pids = after.matching_pids(app_filter);
                Ok((after, after_pids))
            },
        )?;
        windows.map(|mut windows| {
            narrow_to_real_windows(&mut windows);
            windows
        })
    })
}

/// What one scoped read saw: the AppKit snapshot, the pids it selected, and
/// the CoreGraphics records read for them.
struct ScopedRead<'a, S> {
    owners: &'a S,
    selected: &'a rustc_hash::FxHashSet<i32>,
    records: &'a [cg_window::WindowRecord],
}

/// Confirms, after a scoped read, that every selected owner is still the
/// process the AppKit snapshot named, so an owner that exited or relaunched
/// during the read makes the read retry instead of reporting it windowless.
/// Libproc answers this per selected pid. When it cannot answer for one, a
/// second AppKit snapshot decides, as it always did before.
fn validate_scoped_owners<S: OwnerSnapshotView>(
    read: ScopedRead<'_, S>,
    mut still_running: impl FnMut(i32, OwnerGeneration) -> Result<bool, AdapterError>,
    recapture: impl FnOnce() -> Result<(S, rustc_hash::FxHashSet<i32>), AdapterError>,
) -> Result<(), AdapterError> {
    let mut pids = read.selected.iter().copied().collect::<Vec<_>>();
    pids.sort_unstable();
    let mut unconfirmed = false;
    for pid in pids {
        let changed = match read
            .owners
            .generation(pid)
            .map(|generation| still_running(pid, generation))
        {
            Some(Ok(running)) => !running,
            Some(Err(_)) => {
                unconfirmed = true;
                false
            }
            None => true,
        };
        if changed {
            return Err(
                crate::system::window::window_inventory_global::owner_churn_error(
                    Some(pid),
                    "selected_owner_changed",
                ),
            );
        }
    }
    if unconfirmed {
        let (after, after_pids) = recapture()?;
        return crate::system::window::window_inventory_global::validate_scoped_snapshot_pair(
            read.owners,
            &after,
            read.selected,
            &after_pids,
            read.records,
        );
    }
    crate::system::window::window_inventory_global::validate_record_generations(
        read.records,
        read.owners,
    )
}

fn owner_still_running(pid: i32, generation: OwnerGeneration) -> Result<bool, AdapterError> {
    let identity = crate::system::process::process_identity::ProcessIdentity::capture(pid)?;
    Ok(identity.is_some_and(|identity| match generation {
        OwnerGeneration::LaunchTime(launch_time) => identity.matches_launch_time(launch_time),
        OwnerGeneration::LiveProcess => true,
    }))
}

pub(crate) fn focused_window_until(deadline: Instant) -> Result<Option<WindowInfo>, AdapterError> {
    let Some(pid) = focused_application_pid(deadline)? else {
        return Ok(None);
    };
    let records =
        cg_window::window_records_until(deadline, cg_window::WindowRecordScope::Pid(pid))?;
    let windows = windows_from_records_with_focus(
        records,
        true,
        |owner| crate::system::window::window_ax_state::read_until(owner, deadline),
        crate::system::process::process_identity::matches_instance,
    )?;
    if focused_application_pid(deadline)? != Some(pid) {
        return Err(AdapterError::app_unresponsive(
            "Foreground application changed during focus verification",
        ));
    }
    Ok(windows.into_iter().next())
}

fn focused_application_pid(deadline: Instant) -> Result<Option<i32>, AdapterError> {
    let system = crate::tree::system_wide::element();
    crate::tree::surface_read::element(&system, "AXFocusedApplication", deadline)?
        .map(|app| crate::tree::properties::ax_ipc::pid(&app, deadline))
        .transpose()
        .map_err(|error| crate::system::focus::map_ax_read_error(error, "read focused application"))
}

fn require_running_app(
    pids: &rustc_hash::FxHashSet<i32>,
    app_filter: &str,
) -> Result<(), AdapterError> {
    if !pids.is_empty() {
        return Ok(());
    }
    Err(AdapterError::new(
        agent_desktop_core::ErrorCode::AppNotFound,
        format!("No running application matches '{app_filter}'"),
    )
    .with_disposition(agent_desktop_core::DeliverySemantics::not_delivered())
    .with_suggestion("Run 'list-apps' to see running applications, or launch the app first."))
}

fn focus_state(
    ax_state: &mut impl FnMut(
        i32,
    ) -> Result<
        crate::system::window::window_ax_state::WindowAxState,
        AdapterError,
    >,
    pid: i32,
    focused_only: bool,
    has_visible_window: bool,
) -> Result<crate::system::window::window_ax_state::WindowAxState, AdapterError> {
    match ax_state(pid) {
        Ok(state) => Ok(state),
        Err(_) if !focused_only && has_visible_window => {
            Ok(crate::system::window::window_ax_state::WindowAxState {
                focused: None,
                minimized_by_id: rustc_hash::FxHashMap::default(),
                accessible_window_ids: rustc_hash::FxHashSet::default(),
                titles: Default::default(),
            })
        }
        Err(error) => Err(error),
    }
}

fn visible_window_pids(records: &[cg_window::WindowRecord]) -> rustc_hash::FxHashSet<i32> {
    records
        .iter()
        .filter(|record| record.presence.is_onscreen())
        .map(|record| record.pid)
        .collect()
}

fn narrow_to_real_windows(windows: &mut Vec<WindowInfo>) {
    windows.retain(|window| window.state.visible == Some(true) || window.state.minimized.is_some());
}

/// The visible or minimized windows a process owns, read once, with their
/// focus. Choosing among several is the caller's: a launch claims one only
/// when it is the only window or the only focused one.
pub(crate) fn real_windows_for_pid_until(
    pid: i32,
    deadline: Instant,
) -> Result<Vec<WindowInfo>, AdapterError> {
    let records =
        cg_window::window_records_until(deadline, cg_window::WindowRecordScope::Pid(pid))?;
    let mut windows = windows_from_records_with_focus(
        records,
        false,
        |owner_pid| crate::system::window::window_ax_state::read_until(owner_pid, deadline),
        |owner_pid, instance| {
            crate::system::process::process_identity::matches_instance(owner_pid, instance)
        },
    )?;
    narrow_to_real_windows(&mut windows);
    Ok(windows)
}

fn windows_from_records_with_focus(
    records: Vec<cg_window::WindowRecord>,
    focused_only: bool,
    mut ax_state: impl FnMut(
        i32,
    ) -> Result<
        crate::system::window::window_ax_state::WindowAxState,
        AdapterError,
    >,
    mut verify_instance: impl FnMut(i32, &str) -> Result<bool, AdapterError>,
) -> Result<Vec<WindowInfo>, AdapterError> {
    let candidates = records
        .into_iter()
        .filter(|record| record.window_number > 0)
        .collect::<Vec<_>>();
    let visible_pids = visible_window_pids(&candidates);
    let mut state_cache = std::collections::HashMap::new();
    let mut joined = Vec::with_capacity(candidates.len());
    for mut record in candidates {
        let process_instance = record.process_instance.clone().ok_or_else(|| {
            identity_race_error(
                record.pid,
                "CoreGraphics record omitted its process instance",
            )
        })?;
        if !verify_instance(record.pid, &process_instance)? {
            return Err(identity_race_error(
                record.pid,
                "window owner changed before accessibility focus read",
            ));
        }
        let state = match state_cache.entry(record.pid) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => entry.insert(focus_state(
                &mut ax_state,
                record.pid,
                focused_only,
                visible_pids.contains(&record.pid),
            )?),
        };
        if !verify_instance(record.pid, &process_instance)? {
            return Err(identity_race_error(
                record.pid,
                "window owner changed after accessibility focus read",
            ));
        }
        state.titles.fill_missing(&mut record);
        joined.push(record);
    }
    windows_from_joined_records(joined, &state_cache, focused_only)
}

fn windows_from_joined_records(
    records: Vec<cg_window::WindowRecord>,
    states: &std::collections::HashMap<i32, crate::system::window::window_ax_state::WindowAxState>,
    focused_only: bool,
) -> Result<Vec<WindowInfo>, AdapterError> {
    let mut title_counts = std::collections::HashMap::new();
    for record in &records {
        if let Some(title) = record.title.clone() {
            *title_counts.entry((record.pid, title)).or_insert(0) += 1;
        }
    }
    let mut windows = Vec::new();
    let mut focused_seen = false;
    for record in records {
        let state = states.get(&record.pid).ok_or_else(|| {
            identity_race_error(record.pid, "accessibility state missing for joined window")
        })?;
        let title_count = record
            .title
            .clone()
            .and_then(|title| title_counts.get(&(record.pid, title)).copied())
            .unwrap_or(0);
        let is_focused = !focused_seen
            && matches_focused_window(
                record.title.as_deref(),
                record.window_number,
                &state.focused,
                title_count,
            );
        if focused_only && !is_focused {
            continue;
        }
        focused_seen |= is_focused;
        let minimized = state.minimized_by_id.get(&record.window_number).copied();
        let accessible = state.accessible_window_ids.contains(&record.window_number);
        windows.push(record.into_window_info(is_focused, minimized, accessible)?);
    }
    Ok(windows)
}

#[cfg(test)]
fn matches_app_filter(app_name: &str, app_filter: &str) -> bool {
    app_filter.is_empty() || agent_desktop_core::app_name_matches(app_name, app_filter)
}

fn identity_race_error(pid: i32, phase: &str) -> AdapterError {
    AdapterError::new(
        agent_desktop_core::ErrorCode::AppUnresponsive,
        "Window owner changed while exact window identity was being assembled",
    )
    .with_details(serde_json::json!({
        "kind": "window_identity_race",
        "pid": pid,
        "phase": phase,
        "complete": false,
        "retryable": true,
    }))
}

type FocusedWindowIdentity = Option<(Option<String>, Option<i64>)>;

fn matches_focused_window(
    title: Option<&str>,
    window_number: i64,
    identity: &FocusedWindowIdentity,
    same_title_count: usize,
) -> bool {
    let Some((focused_title, focused_number)) = identity else {
        return false;
    };
    if let Some(number) = focused_number {
        return *number == window_number;
    }
    focused_title.is_some() && focused_title.as_deref() == title && same_title_count == 1
}

#[cfg(test)]
#[path = "window_inventory_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "window_inventory_title_tests.rs"]
mod title_tests;

#[cfg(test)]
#[path = "window_inventory_scoped_tests.rs"]
mod scoped_tests;
