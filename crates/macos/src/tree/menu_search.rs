//! The breadth-first search for a displayed menu below an element.

use super::surface_read;
use agent_desktop_core::{AdapterError, ErrorCode};
use std::time::Instant;

pub(super) const MAX_SURFACE_NODES: usize = 2_048;

/// The search for a context menu walks the focused element first and then
/// every window, which holds the focused element again. A subtree that an
/// earlier pass of the same search finished without finding a menu is not
/// read twice: `already_searched` names its root, which is then treated as a
/// leaf. Only a pass that finished is a proof, because an incomplete one fails
/// the whole search before the next pass starts.
pub(super) fn outside_searched<T>(
    already_searched: impl Fn(&T) -> bool,
    mut inspect: impl FnMut(&T) -> Result<(bool, Vec<T>), AdapterError>,
) -> impl FnMut(&T) -> Result<(bool, Vec<T>), AdapterError> {
    move |element| {
        if already_searched(element) {
            return Ok((false, Vec::new()));
        }
        inspect(element)
    }
}

pub(super) fn find_menu_descendant_with<T>(
    root: T,
    source: String,
    deadline: Instant,
    mut inspect: impl FnMut(&T) -> Result<(bool, Vec<T>), AdapterError>,
) -> Result<Option<(String, T)>, AdapterError> {
    let mut pending = std::collections::VecDeque::from([(source, root)]);
    let mut incomplete = None;
    let mut visited = 0_usize;
    while let Some((path, element)) = pending.pop_front() {
        surface_read::ensure_before_deadline(deadline)?;
        visited += 1;
        if visited > MAX_SURFACE_NODES {
            return Err(surface_limit_error());
        }
        let (is_menu, children) = match inspect(&element) {
            Ok(observed) => observed,
            Err(error)
                if error.code == ErrorCode::AppUnresponsive
                    && !super::app_menu::invalidated(&error) =>
            {
                incomplete.get_or_insert(error);
                continue;
            }
            Err(error) => return Err(error),
        };
        if is_menu {
            return Ok(Some((path, element)));
        }
        pending.extend(
            children
                .into_iter()
                .enumerate()
                .map(|(index, child)| (format!("{path}/children/{index}"), child)),
        );
    }
    incomplete.map_or(Ok(None), Err)
}

pub(super) fn surface_limit_error() -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Accessibility surface search exceeded its node budget",
    )
    .with_details(serde_json::json!({
        "kind": "surface_search_limit",
        "limit": MAX_SURFACE_NODES,
        "complete": false,
        "retryable": false,
    }))
    .with_suggestion("Retry with the target application in a more stable UI state")
}

#[cfg(test)]
#[path = "menu_search_tests.rs"]
mod tests;
