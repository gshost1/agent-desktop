//! Global inventory in two halves. `capture_frontmost_focus` runs inside the
//! owner-snapshot bracket and reads only the frontmost owner's focus.
//! `finish_global_windows` runs after the bracket, once the owner probe has
//! read titles and accessible ids, so the focus join compares the same titles
//! the listing reports.

use super::{OwnerSnapshotView, validate_record_generations};
use crate::system::window::{cg_window::WindowRecord, window_ax_state::WindowAxState};
use agent_desktop_core::{AdapterError, ErrorCode, WindowInfo};
use rustc_hash::FxHashMap;

pub(super) struct GlobalCapture {
    pub(super) records: Vec<WindowRecord>,
    frontmost_pid: Option<i32>,
    focus_state: Option<WindowAxState>,
}

impl GlobalCapture {
    /// A focused-only listing reports at most the frontmost owner's window, so
    /// only that owner is probed, and only when it reported a focused window.
    pub(super) fn probe_pids(&self, focused_only: bool) -> Vec<i32> {
        if focused_only {
            let focus_reported = self
                .focus_state
                .as_ref()
                .is_some_and(|state| state.focused.is_some());
            return self
                .frontmost_pid
                .filter(|_| focus_reported)
                .into_iter()
                .collect();
        }
        let mut pids = self
            .records
            .iter()
            .map(|record| record.pid)
            .collect::<Vec<_>>();
        pids.sort_unstable();
        pids.dedup();
        pids
    }
}

pub(super) fn capture_frontmost_focus<S: OwnerSnapshotView>(
    records: Vec<WindowRecord>,
    owners: &S,
    mut read_frontmost: impl FnMut(i32) -> Result<WindowAxState, AdapterError>,
) -> Result<GlobalCapture, AdapterError> {
    let records = records
        .into_iter()
        .filter(|record| record.window_number > 0)
        .collect::<Vec<_>>();
    validate_record_generations(&records, owners)?;
    let frontmost_pid = owners.frontmost_pid();
    let focus_state = match frontmost_pid.filter(|pid| records.iter().any(|r| r.pid == *pid)) {
        Some(pid) => Some(read_frontmost(pid).map_err(frontmost_read_error)?),
        None => None,
    };
    Ok(GlobalCapture {
        records,
        frontmost_pid,
        focus_state,
    })
}

/// An owner missing from `states` was not probed or its probe failed: its
/// windows keep their CoreGraphics titles and stay selectable, because an
/// unknown accessibility label is not evidence that a window is inaccessible.
pub(super) fn finish_global_windows(
    capture: GlobalCapture,
    states: &FxHashMap<i32, WindowAxState>,
    focused_only: bool,
) -> Result<Vec<WindowInfo>, AdapterError> {
    let GlobalCapture {
        mut records,
        frontmost_pid,
        focus_state,
    } = capture;
    for record in &mut records {
        crate::system::window::window_title::join_ax_title(record, states.get(&record.pid));
    }
    let focus_reported = focus_state
        .as_ref()
        .is_some_and(|state| state.focused.is_some());
    let focused_window = match matching_focus_windows(&records, frontmost_pid, focus_state.as_ref())
    {
        FocusJoin::TitleAmbiguous => None,
        FocusJoin::Windows(windows) => {
            if focus_reported && windows.len() != 1 {
                return Err(focus_join_error(frontmost_pid, windows.len()));
            }
            windows.first().copied()
        }
    };
    records
        .into_iter()
        .filter_map(|record| {
            let is_focused = Some((record.pid, record.window_number)) == focused_window;
            if focused_only && !is_focused {
                return None;
            }
            let minimized = if Some(record.pid) == frontmost_pid {
                focus_state
                    .as_ref()
                    .and_then(|state| state.minimized_by_id.get(&record.window_number).copied())
            } else {
                None
            };
            let accessible = states
                .get(&record.pid)
                .is_none_or(|state| state.accessible_window_ids.contains(&record.window_number));
            Some(record.into_window_info(is_focused, minimized, accessible))
        })
        .collect::<Result<Vec<_>, _>>()
}

enum FocusJoin {
    Windows(Vec<(i32, i64)>),
    TitleAmbiguous,
}

fn matching_focus_windows(
    records: &[WindowRecord],
    frontmost_pid: Option<i32>,
    state: Option<&WindowAxState>,
) -> FocusJoin {
    let Some(frontmost_pid) = frontmost_pid else {
        return FocusJoin::Windows(Vec::new());
    };
    let Some((focused_title, focused_number)) = state.and_then(|state| state.focused.as_ref())
    else {
        return FocusJoin::Windows(Vec::new());
    };
    if let Some(number) = focused_number {
        return FocusJoin::Windows(
            records
                .iter()
                .filter(|record| record.pid == frontmost_pid && record.window_number == *number)
                .map(|record| (record.pid, record.window_number))
                .collect(),
        );
    }
    let title_matches = records
        .iter()
        .filter(|record| {
            record.pid == frontmost_pid && focused_title.as_deref() == Some(record.display_title())
        })
        .map(|record| (record.pid, record.window_number))
        .collect::<Vec<_>>();
    if title_matches.len() > 1 {
        return FocusJoin::TitleAmbiguous;
    }
    FocusJoin::Windows(title_matches)
}

fn frontmost_read_error(error: AdapterError) -> AdapterError {
    if !matches!(
        error.code,
        ErrorCode::Timeout | ErrorCode::AppUnresponsive | ErrorCode::ElementNotFound
    ) {
        return error;
    }
    let source_kind = error
        .details
        .as_ref()
        .and_then(|details| details.get("kind"))
        .cloned();
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Authoritative frontmost window accessibility read was incomplete",
    )
    .with_details(serde_json::json!({
        "kind": "frontmost_window_ax_incomplete",
        "source_code": error.code.as_str(),
        "source_kind": source_kind,
        "complete": false,
        "retryable": true,
    }))
}

fn focus_join_error(pid: Option<i32>, match_count: usize) -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Authoritative frontmost app did not join to one exact window",
    )
    .with_details(serde_json::json!({
        "kind": "frontmost_window_join",
        "pid": pid,
        "match_count": match_count,
        "complete": false,
        "retryable": true,
    }))
}
