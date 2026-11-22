use agent_desktop_core::{AdapterError, ErrorCode, WindowInfo};

use crate::system::window::cg_window::WindowRecord;
use crate::system::window::window_resolve_errors::{
    invalid_window_id, window_identity_mismatch, window_not_accessible, window_not_found,
};
use crate::system::window::window_sources::{LiveWindowSources, WindowSources};
use crate::tree::{AXElement, properties::attributes::set_messaging_timeout};

pub(crate) struct WindowIdentityEvidence<'a> {
    pub pid: i32,
    pub app: Option<&'a str>,
    pub process_instance: Option<&'a str>,
    pub title: Option<&'a str>,
    pub bounds_hash: Option<u64>,
}

pub(crate) fn window_element_for_info(
    win: &WindowInfo,
    deadline: agent_desktop_core::Deadline,
) -> Result<AXElement, AdapterError> {
    window_element_for_info_with_deadline(
        win,
        std::time::Instant::now()
            .checked_add(deadline.remaining())
            .ok_or_else(|| AdapterError::new(ErrorCode::InvalidArgs, "deadline is out of range"))?,
    )
}

pub(crate) fn window_element_for_info_with_deadline(
    win: &WindowInfo,
    deadline: std::time::Instant,
) -> Result<AXElement, AdapterError> {
    if win.is_process_owner() && win.title.is_empty() {
        return Err(AdapterError::app_has_no_window(&win.app));
    }
    if win.id.is_empty() {
        let pid = crate::system::process::process_identity::to_pid_t(win.pid)?;
        return crate::system::window::window_element_lookup::window_element_by_title(
            pid, &win.title, deadline,
        );
    }
    resolve_window_element_strict(win, deadline, &LiveWindowSources)
}

pub(crate) fn resolve_capture_window(
    win: &WindowInfo,
    deadline: std::time::Instant,
) -> Result<WindowInfo, AdapterError> {
    let (_, record) = locate_verified_record_until(win, deadline, &LiveWindowSources)?;
    window_info_from_record(win, &record)
}

pub(crate) fn resolve_window_strict(
    win: &WindowInfo,
    deadline: std::time::Instant,
) -> Result<WindowInfo, AdapterError> {
    resolve_window_strict_with(win, deadline, &LiveWindowSources)
}

fn resolve_window_strict_with(
    win: &WindowInfo,
    deadline: std::time::Instant,
    sources: &impl WindowSources,
) -> Result<WindowInfo, AdapterError> {
    let (window_number, mut record) = locate_verified_record_until(win, deadline, sources)?;
    let ax_state = sources.ax_state(record.pid, deadline)?;
    verify_window_record(win, &record)?;
    crate::system::window::window_liveness::require_live_window(
        &win.id,
        &record,
        &ax_state,
        |number| sources.is_on_any_space(number),
    )?;
    ax_state.titles.fill_missing(&mut record);
    let mut resolved = window_info_from_record(win, &record)?;
    resolved.state.minimized = ax_state.minimized_by_id.get(&window_number).copied();
    if let Some((_, Some(focused_number))) = ax_state.focused {
        resolved.state.is_focused = focused_number == window_number;
    }
    Ok(resolved)
}

fn locate_verified_record_until(
    win: &WindowInfo,
    deadline: std::time::Instant,
    sources: &impl WindowSources,
) -> Result<(i64, WindowRecord), AdapterError> {
    let window_number = parse_window_number(&win.id).ok_or_else(|| invalid_window_id(&win.id))?;
    let record = sources
        .record(window_number, deadline)?
        .ok_or_else(|| window_not_found(&win.id))?;
    verify_window_record(win, &record)?;
    Ok((window_number, record))
}

pub(crate) fn verify_window_identity_until(
    id: &str,
    evidence: WindowIdentityEvidence<'_>,
    deadline: std::time::Instant,
) -> Result<(), AdapterError> {
    let window_number = parse_window_number(id).ok_or_else(|| invalid_window_id(id))?;
    let record =
        crate::system::window::cg_window_exact::exact_window_record_until(window_number, deadline)?
            .ok_or_else(|| window_not_found(id))?;
    if !window_record_matches_source(&record, &evidence) {
        return Err(window_identity_mismatch(id));
    }
    let Some(process_instance) = evidence.process_instance else {
        return Err(window_identity_mismatch(id));
    };
    if !crate::system::process::process_identity::matches_instance(evidence.pid, process_instance)?
    {
        return Err(window_identity_mismatch(id));
    }
    Ok(())
}

fn window_record_matches_source(
    record: &WindowRecord,
    evidence: &WindowIdentityEvidence<'_>,
) -> bool {
    if record.pid != evidence.pid
        || evidence
            .app
            .is_some_and(|app| !app.is_empty() && record.app_name != app)
        || evidence.process_instance.is_none()
        || record.process_instance.as_deref() != evidence.process_instance
    {
        return false;
    }
    let bounds_changed = evidence.bounds_hash.is_some_and(|expected| {
        record
            .bounds
            .bounds_hash()
            .is_none_or(|actual| actual != expected)
    });
    if bounds_changed {
        tracing::debug!(
            expected_bounds_hash = ?evidence.bounds_hash,
            actual_bounds_hash = ?record.bounds.bounds_hash(),
            "window moved or resized while immutable source identity remained valid"
        );
    }
    let title_changed = evidence.title.is_some_and(|title| {
        !title.is_empty()
            && record
                .title
                .as_deref()
                .is_some_and(|recorded| recorded != title)
    });
    if title_changed {
        tracing::debug!("window title changed while immutable source identity remained valid");
    }
    true
}

fn resolve_window_element_strict(
    win: &WindowInfo,
    deadline: std::time::Instant,
    sources: &impl WindowSources,
) -> Result<AXElement, AdapterError> {
    crate::tree::locator_deadline::remaining(deadline)?;
    let (window_number, record) = locate_verified_record_until(win, deadline, sources)?;
    crate::tree::locator_deadline::remaining(deadline)?;
    let pid = crate::system::process::process_identity::to_pid_t(win.pid)?;
    if let Some(element) = sources.element(pid, window_number, record.title.as_deref(), deadline)? {
        return Ok(element);
    }
    if !record.presence.is_onscreen() {
        let state = sources.ax_state(pid, deadline)?;
        crate::system::window::window_liveness::require_live_window(
            &win.id,
            &record,
            &state,
            |number| sources.is_on_any_space(number),
        )?;
    }
    Err(window_not_accessible(&win.id))
}

pub(crate) fn parse_window_number(id: &str) -> Option<i64> {
    id.strip_prefix('w')?
        .strip_prefix('-')?
        .parse()
        .ok()
        .filter(|number| *number > 0)
}

fn verify_window_record(win: &WindowInfo, record: &WindowRecord) -> Result<(), AdapterError> {
    let pid = crate::system::process::process_identity::to_pid_t(win.pid)?;
    let evidence = WindowIdentityEvidence {
        pid,
        app: Some(&win.app),
        process_instance: win.process_instance.as_deref(),
        title: Some(&win.title),
        bounds_hash: None,
    };
    if !window_record_matches_source(record, &evidence) {
        return Err(window_identity_mismatch(&win.id));
    }
    let Some(instance) = evidence.process_instance else {
        return Err(window_identity_mismatch(&win.id));
    };
    if !crate::system::process::process_identity::matches_instance(pid, instance)? {
        return Err(window_identity_mismatch(&win.id));
    }
    Ok(())
}

fn window_info_from_record(
    win: &WindowInfo,
    record: &WindowRecord,
) -> Result<WindowInfo, AdapterError> {
    Ok(WindowInfo {
        id: win.id.clone(),
        title: record.title.clone().unwrap_or_default(),
        app: record.app_name.clone(),
        pid: crate::system::process::process_identity::from_pid_t(record.pid)?,
        process_instance: record.process_instance.clone(),
        bounds: Some(record.bounds),
        state: agent_desktop_core::WindowState {
            is_focused: win.state.is_focused,
            accessible: win.state.accessible,
            minimized: win.state.minimized,
            visible: Some(record.presence.is_onscreen()),
        },
    })
}

pub(crate) fn prepare_for_read(
    element: &AXElement,
    deadline: std::time::Instant,
) -> Result<(), AdapterError> {
    crate::tree::locator_deadline::remaining(deadline)?;
    set_messaging_timeout(element, deadline)?;
    Ok(())
}

pub(crate) fn ax_window_id_with_deadline(
    window: &AXElement,
    deadline: std::time::Instant,
) -> Result<Option<i64>, AdapterError> {
    prepare_for_read(window, deadline)?;
    let window_id = crate::system::window::window_bridge::window_id(window, deadline)?;
    crate::tree::locator_deadline::remaining(deadline)?;
    Ok(window_id)
}

#[cfg(test)]
#[path = "window_resolve_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "window_resolve_liveness_tests.rs"]
mod liveness_tests;
