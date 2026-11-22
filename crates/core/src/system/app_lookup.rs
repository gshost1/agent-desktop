use std::time::Duration;

use serde_json::json;

use crate::{
    AdapterError, AppError, AppInfo, AppInventory, AppInventoryQuery, Deadline, ErrorCode,
    ProcessIdentity, WindowInfo,
    adapter::{PlatformAdapter, WindowFilter},
};

/// Owner for a named app that lists no window. A background app is observed
/// through its process; any other app has not presented a window yet. Failures
/// of the app lookup propagate, except an adapter without an app inventory,
/// which keeps the window error.
pub(crate) fn windowless_app_owner(
    adapter: &dyn PlatformAdapter,
    app: &str,
    deadline: Deadline,
) -> Result<WindowInfo, AppError> {
    match resolve_app(Some(app), adapter, deadline) {
        Ok(info) if info.presentation == Some(crate::AppPresentation::Background) => {
            Ok(WindowInfo::for_process(&info))
        }
        Ok(_) => Err(AdapterError::app_has_no_window(app).into()),
        Err(AppError::Adapter(error)) if error.code == ErrorCode::PlatformNotSupported => {
            Err(AdapterError::app_has_no_window(app).into())
        }
        Err(error) => Err(error),
    }
}

/// Owner of the web surface for a named app that lists no window. The web
/// surface is read through the process's DevTools endpoint and needs no window,
/// so a window the inventory cannot see, such as one on another Space, does not
/// block it. Only one running process with an endpoint `launch --cdp` recorded
/// for its exact instance qualifies; anything else answers `None` and the
/// caller keeps its window error.
pub(crate) fn web_process_owner(
    adapter: &dyn PlatformAdapter,
    app: &str,
    deadline: Deadline,
) -> Option<WindowInfo> {
    let info = resolve_app(Some(app), adapter, deadline).ok()?;
    let process = process_identity(&info).ok()?;
    adapter
        .has_recorded_cdp_endpoint(process)
        .then(|| WindowInfo::for_process(&info))
}

pub(crate) fn resolve_app(
    app: Option<&str>,
    adapter: &dyn PlatformAdapter,
    deadline: Deadline,
) -> Result<AppInfo, AppError> {
    if let Some(name) = app {
        let candidates = exact_apps(adapter, name, None, deadline)?;
        if candidates.is_empty() {
            return Err(with_app_name_suggestions(
                exact_app_not_found(name),
                name,
                deadline,
                |query, deadline| adapter.query_app_inventory(query, deadline),
            )
            .into());
        }
        return select_unique_app(candidates, name);
    }
    let focused = adapter.list_windows(
        &WindowFilter {
            focused_only: true,
            app: None,
        },
        deadline,
    )?;
    let window = match focused.as_slice() {
        [] => {
            return Err(AdapterError::new(
                ErrorCode::AppNotFound,
                "No focused application was found when --app was omitted",
            )
            .unsent()
            .into());
        }
        [window] => window,
        _ => {
            return Err(AdapterError::ambiguous_target(
                "A unique focused window is required when --app is omitted",
            )
            .with_details(json!({ "focused_window_count": focused.len() }))
            .into());
        }
    };
    resolve_app_owning(window, adapter, deadline)
}

/// The application that owns `window`, narrowed to the window's exact process
/// instance, so a second running copy of the same application is never chosen.
pub(crate) fn resolve_app_owning(
    window: &WindowInfo,
    adapter: &dyn PlatformAdapter,
    deadline: Deadline,
) -> Result<AppInfo, AppError> {
    let instance = window
        .process_instance
        .as_deref()
        .filter(|instance| !instance.is_empty())
        .ok_or_else(|| {
            AdapterError::new(
                ErrorCode::ActionNotSupported,
                "Window has no process-instance identity",
            )
        })?;
    let same_pid = exact_apps(adapter, &window.app, None, deadline)?
        .into_iter()
        .filter(|candidate| candidate.pid == window.pid)
        .collect::<Vec<_>>();
    reject_incomplete_app_identity(&same_pid, &window.app)?;
    let candidates = same_pid
        .into_iter()
        .filter(|candidate| candidate.process_instance.as_deref() == Some(instance))
        .collect();
    select_unique_app(candidates, &window.app)
}

fn select_unique_app(mut candidates: Vec<AppInfo>, label: &str) -> Result<AppInfo, AppError> {
    reject_incomplete_app_identity(&candidates, label)?;
    match candidates.len() {
        0 => Err(exact_app_not_found(label).into()),
        1 => Ok(candidates.swap_remove(0)),
        _ => {
            let candidate_pids = candidates.iter().map(|c| c.pid).collect::<Vec<_>>();
            let summaries = candidates
                .iter()
                .take(10)
                .map(|candidate| {
                    json!({
                        "name": candidate.name,
                        "pid": candidate.pid,
                        "process_instance": candidate.process_instance,
                    })
                })
                .collect::<Vec<_>>();
            Err(AdapterError::ambiguous_process_target_with_details(
                format!("Multiple application instances matched '{label}'"),
                &candidate_pids,
                json!({
                    "candidate_count": candidates.len(),
                    "candidate_pids": candidate_pids,
                    "candidate_summaries_truncated": candidates.len() > summaries.len(),
                    "candidates": summaries,
                }),
            )
            .into())
        }
    }
}

fn exact_app_not_found(label: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::AppNotFound,
        format!("Application '{label}' was not found with exact process identity"),
    )
    .with_suggestion("Application name as reported by list-apps")
    .unsent()
}

pub(crate) fn is_near_app_name(actual: &str, query: &str) -> bool {
    if query.trim().is_empty() {
        return false;
    }
    let actual = actual.to_lowercase();
    let query = query.to_lowercase();
    actual.starts_with(&query) || app_name_stem(&actual) == app_name_stem(&query)
}

fn app_name_stem(name: &str) -> &str {
    let name = name.strip_suffix(".bin").unwrap_or(name);
    let stem = name
        .rsplit_once(['-', '_', ' '])
        .filter(|(_, version)| is_app_version(version.strip_prefix('v').unwrap_or(version)))
        .map(|(stem, _)| stem)
        .or_else(|| {
            let stem = name.trim_end_matches(|ch: char| ch.is_ascii_digit() || ch == '.');
            let version = &name[stem.len()..];
            let version = version.strip_prefix('.').unwrap_or(version);
            (version.contains('.') && is_app_version(version)).then_some(stem)
        })
        .filter(|stem| !stem.is_empty())
        .unwrap_or(name);
    stem.strip_suffix(".bin").unwrap_or(stem)
}

fn is_app_version(suffix: &str) -> bool {
    suffix
        .split('.')
        .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

/// Best-effort diagnostics after an exact name refusal; never supplies an action target.
pub fn with_app_name_suggestions(
    mut error: AdapterError,
    name: &str,
    deadline: Deadline,
    lookup: impl FnOnce(AppInventoryQuery<'_>, Deadline) -> Result<AppInventory, AdapterError>,
) -> AdapterError {
    let deadline = deadline.capped(Duration::from_millis(20));
    if error.code != ErrorCode::AppNotFound || name.trim().is_empty() || deadline.is_expired() {
        return error;
    }
    let Ok(mut inventory) = lookup(AppInventoryQuery::NearMatches(name), deadline) else {
        return error;
    };
    if inventory.apps.is_empty() {
        return error;
    }
    inventory
        .apps
        .sort_by(|left, right| (&left.name, left.pid).cmp(&(&right.name, right.pid)));
    let suggestions = inventory
        .apps
        .iter()
        .map(|app| {
            let name = &app.name;
            let argument = if name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
                && !name.starts_with('-')
            {
                format!("--app {name}")
            } else {
                format!("--app='{}'", name.replace('\'', "'\\''"))
            };
            format!("'{name}' (pid {})? retry with {argument}", app.pid.get())
        })
        .collect::<Vec<_>>();
    let candidates = inventory
        .apps
        .iter()
        .map(|app| {
            json!({
                "name": app.name,
                "pid": app.pid,
            })
        })
        .collect::<Vec<_>>();
    let mut details = error.details.take().unwrap_or_else(|| json!({}));
    if let Some(object) = details.as_object_mut() {
        object.insert("candidates".into(), json!(candidates));
    }
    error
        .with_suggestion(format!("did you mean {}", suggestions.join("; or ")))
        .with_details(details)
}

fn reject_incomplete_app_identity(candidates: &[AppInfo], label: &str) -> Result<(), AppError> {
    let incomplete_count = candidates
        .iter()
        .filter(|candidate| {
            candidate
                .process_instance
                .as_deref()
                .is_none_or(str::is_empty)
        })
        .count();
    if incomplete_count == 0 {
        return Ok(());
    }
    Err(AdapterError::new(
        ErrorCode::ActionNotSupported,
        format!("Application inventory for '{label}' has incomplete process identity"),
    )
    .with_details(json!({
        "candidate_count": candidates.len(),
        "incomplete_identity_count": incomplete_count,
    }))
    .into())
}

pub(crate) fn process_identity(app: &AppInfo) -> Result<ProcessIdentity, AppError> {
    let instance = app
        .process_instance
        .as_deref()
        .filter(|instance| !instance.is_empty())
        .ok_or_else(|| {
            AdapterError::new(
                ErrorCode::ActionNotSupported,
                "Application has no process-instance identity",
            )
        })?;
    Ok(ProcessIdentity::new(app.pid, instance))
}

/// The live process a process-owned ref was captured from, revalidated by pid,
/// process instance and name; a replaced or exited process is stale.
pub(crate) fn live_process_owner(
    adapter: &dyn PlatformAdapter,
    entry: &crate::RefEntry,
    deadline: Deadline,
) -> Result<WindowInfo, AppError> {
    let stale = |reason| AppError::Adapter(AdapterError::stale_ref_because(reason));
    let expected = AppInfo {
        name: entry
            .source
            .source_app
            .clone()
            .ok_or_else(|| stale("ref has no source app"))?,
        pid: entry.process.pid,
        bundle_id: None,
        process_instance: Some(
            entry
                .process
                .process_instance
                .clone()
                .ok_or_else(|| stale("ref has no process instance"))?,
        ),
        presentation: None,
    };
    Ok(WindowInfo::for_process(&revalidate_app_for_mutation(
        adapter, &expected, deadline,
    )?))
}

pub(crate) fn revalidate_app_for_mutation(
    adapter: &dyn PlatformAdapter,
    expected: &AppInfo,
    deadline: Deadline,
) -> Result<AppInfo, AppError> {
    let expected_identity = process_identity(expected)?;
    let same_pid = exact_apps(
        adapter,
        &expected.name,
        expected.bundle_id.as_deref(),
        deadline,
    )?
    .into_iter()
    .filter(|candidate| candidate.pid == expected.pid)
    .collect::<Vec<_>>();
    reject_incomplete_app_identity(&same_pid, &expected.name)?;
    let mut exact = same_pid
        .into_iter()
        .filter(|candidate| {
            candidate.process_instance.as_deref() == Some(expected_identity.instance.as_str())
                && crate::app_name_matches(&candidate.name, &expected.name)
                && expected.bundle_id.as_deref().is_none_or(|expected_bundle| {
                    candidate
                        .bundle_id
                        .as_deref()
                        .is_some_and(|actual| actual.eq_ignore_ascii_case(expected_bundle))
                })
        })
        .collect::<Vec<_>>();
    match exact.len() {
        1 => Ok(exact.swap_remove(0)),
        0 => Err(AdapterError::new(
            ErrorCode::StaleRef,
            "Application identity changed before mutation",
        )
        .into()),
        _ => Err(AdapterError::ambiguous_target(
            "Multiple exact application identities remained before mutation",
        )
        .with_details(json!({ "candidate_count": exact.len() }))
        .into()),
    }
}

fn exact_apps(
    adapter: &dyn PlatformAdapter,
    name: &str,
    bundle_id: Option<&str>,
    deadline: Deadline,
) -> Result<Vec<AppInfo>, AppError> {
    let inventory = adapter.query_app_inventory(
        crate::AppInventoryQuery::Exact { name, bundle_id },
        deadline,
    )?;
    if !inventory.skipped.is_empty() {
        return Err(AdapterError::app_unresponsive(name)
            .with_details(
                json!({ "kind": "app_inventory_incomplete", "skipped": inventory.skipped }),
            )
            .unsent()
            .into());
    }
    Ok(inventory.apps)
}

#[cfg(test)]
#[path = "app_lookup_tests.rs"]
mod tests;
