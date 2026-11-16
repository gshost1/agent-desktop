//! Names an application's main window from what its accessibility tree
//! reports, for an `--app` command that matches several windows of an
//! application no window of which holds focus, as when it is in the
//! background.
//!
//! The application element's `AXMainWindow` and each window's `AXMain` keep
//! naming the window the application would make key when activated, while
//! another application is in front. Two reports must agree before a window is
//! chosen: exactly one window says `AXMain`, the application names that same
//! window, and every candidate's `AXMain` was read. Anything less is no
//! choice, and the caller keeps its ambiguity.

use agent_desktop_core::{Deadline, WindowInfo};
use std::time::Instant;

/// The id of the window among `windows` the application names its main
/// window, or `None` when the reports are missing, unreadable or disagree.
pub(crate) fn main_window(windows: &[WindowInfo], deadline: Deadline) -> Option<String> {
    let pid = one_pid(windows)?;
    let deadline = crate::tree::locator_deadline::from_operation(deadline).ok()?;
    let candidates = windows
        .iter()
        .map(|window| super::window_resolve::parse_window_number(&window.id))
        .collect::<Option<Vec<_>>>()?;
    let app = crate::tree::element_for_pid(pid);
    let named = crate::tree::surface_read::element(&app, "AXMainWindow", deadline).ok()??;
    let named = super::window_resolve::ax_window_id_with_deadline(&named, deadline).ok()??;
    let reports = main_reports(&app, deadline)?;
    decide_main(named, &reports, &candidates).map(|number| format!("w-{number}"))
}

/// Each AX window's window number and its `AXMain`, `None` where it could
/// not be read.
fn main_reports(
    app: &crate::tree::AXElement,
    deadline: Instant,
) -> Option<Vec<(i64, Option<bool>)>> {
    let windows = crate::tree::surface_read::elements(app, "AXWindows", deadline).ok()?;
    let mut reports = Vec::with_capacity(windows.len());
    for window in windows {
        let Ok(Some(number)) = super::window_resolve::ax_window_id_with_deadline(&window, deadline)
        else {
            continue;
        };
        let main = crate::tree::surface_read::boolean(&window, "AXMain", deadline)
            .ok()
            .flatten();
        reports.push((number, main));
    }
    Some(reports)
}

fn one_pid(windows: &[WindowInfo]) -> Option<i32> {
    let first = windows.first()?.pid;
    if windows.iter().any(|window| window.pid != first) {
        return None;
    }
    crate::system::process::process_identity::to_pid_t(first).ok()
}

/// The window number both reports name: the only window whose `AXMain` is
/// true, when it is the application's `named` main window, one of the
/// `candidates`, and every candidate's `AXMain` was read.
pub(crate) fn decide_main(
    named: i64,
    reports: &[(i64, Option<bool>)],
    candidates: &[i64],
) -> Option<i64> {
    let mut mains = reports
        .iter()
        .filter(|(_, main)| *main == Some(true))
        .map(|(number, _)| *number);
    let main = mains.next()?;
    let every_candidate_read = candidates.iter().all(|candidate| {
        reports
            .iter()
            .any(|(number, main)| number == candidate && main.is_some())
    });
    (mains.next().is_none() && main == named && candidates.contains(&main) && every_candidate_read)
        .then_some(main)
}

#[cfg(test)]
#[path = "window_main_tests.rs"]
mod tests;
