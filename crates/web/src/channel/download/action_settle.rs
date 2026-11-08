use std::time::Duration;

use agent_desktop_core::{AdapterError, Deadline};
use serde_json::json;

use super::download_event::is_download_start;
use crate::transport::transport_error::is_timeout;
use crate::{CdpConnection, CdpEvent, PageSession};

const START_WINDOW: Duration = Duration::from_millis(100);
const QUIET: Duration = Duration::from_millis(100);
const SETTLE_CAP: Duration = Duration::from_millis(3_000);

const NAVIGATION_STARTS: &[&str] = &[
    "Page.frameScheduledNavigation",
    "Page.frameRequestedNavigation",
    "Page.frameStartedNavigating",
    "Page.frameStartedLoading",
];
const NAVIGATION_ENDS: &[&str] = &[
    "Page.frameStoppedLoading",
    "Page.loadEventFired",
    "Page.navigatedWithinDocument",
];

/// Turns on the page's navigation events before an action that may start
/// a download, and drops any already buffered, so [`settle_action`] sees
/// only what the action set off.
pub(crate) fn watch_navigation(
    connection: &mut CdpConnection,
    session: &PageSession,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    connection.call(
        "Page.enable",
        json!({}),
        Some(&session.session_id),
        deadline,
    )?;
    connection.discard_events(is_navigation);
    Ok(())
}

/// Waits for what an action set off to settle, after Chrome DevTools MCP's
/// pattern: up to 100 ms for a navigation or a download to start, returning
/// at once when nothing does; once one starts, until a download begins or
/// the page stops loading, then until 100 ms pass with no navigation event
/// or download start, all within 3 s. A server that answers a download
/// link slowly is followed for the whole 3 s. Returns the download starts it
/// took off the connection; download progress stays buffered.
pub(crate) fn settle_action(
    connection: &mut CdpConnection,
    deadline: Deadline,
) -> Result<Vec<CdpEvent>, AdapterError> {
    let cap = deadline.capped(SETTLE_CAP);
    let mut starts = Vec::new();
    let Some(first) = next(connection, is_start, cap.capped(START_WINDOW))? else {
        return Ok(starts);
    };
    keep(first, &mut starts);
    if starts.is_empty() {
        let ended = |event: &CdpEvent| {
            is_download_start(event) || NAVIGATION_ENDS.contains(&event.method.as_str())
        };
        match next(connection, ended, cap)? {
            Some(event) => keep(event, &mut starts),
            None => return Ok(starts),
        }
    }
    let settling = |event: &CdpEvent| is_download_start(event) || is_navigation(event);
    while let Some(event) = next(connection, settling, cap.capped(QUIET))? {
        keep(event, &mut starts);
    }
    Ok(starts)
}

fn next(
    connection: &mut CdpConnection,
    matches: impl FnMut(&CdpEvent) -> bool,
    deadline: Deadline,
) -> Result<Option<CdpEvent>, AdapterError> {
    match connection.wait_for_event(matches, deadline) {
        Ok(event) => Ok(Some(event)),
        Err(error) if is_timeout(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

fn keep(event: CdpEvent, starts: &mut Vec<CdpEvent>) {
    if is_download_start(&event) {
        starts.push(event);
    }
}

fn is_start(event: &CdpEvent) -> bool {
    is_download_start(event) || NAVIGATION_STARTS.contains(&event.method.as_str())
}

fn is_navigation(event: &CdpEvent) -> bool {
    event.session_id.is_some()
        && (NAVIGATION_STARTS.contains(&event.method.as_str())
            || NAVIGATION_ENDS.contains(&event.method.as_str()))
}

#[cfg(test)]
#[path = "action_settle_tests.rs"]
mod tests;
