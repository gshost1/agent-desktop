use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode, WindowInfo};
use serde_json::json;

use super::browser_link::BrowserLink;
use crate::{BrowserWindow, PageTarget, list_page_targets, page_visibility};

const BOUNDS_TOLERANCE: f64 = 2.0;

/// The page target a platform window shows. Targets are grouped by the
/// browser window that holds them; the group whose bounds match the window's
/// accessibility bounds is kept (or the only group, when the browser has one
/// window or the window reports no bounds), and among its tabs the one whose
/// `document.visibilityState` is `visible` is chosen.
pub fn choose_page(
    link: &mut BrowserLink,
    window: Option<&WindowInfo>,
    deadline: Deadline,
) -> Result<PageTarget, AdapterError> {
    let mut placed = Vec::new();
    for target in list_page_targets(&mut link.connection, deadline)? {
        if let Ok(browser_window) =
            BrowserWindow::of_target(&mut link.connection, &target.target_id, deadline)
        {
            placed.push((target, browser_window));
        }
    }
    if placed.is_empty() {
        return Err(no_page("The browser has no open page"));
    }
    let candidates = in_window(placed, window)?;
    if candidates.len() == 1 {
        return candidates
            .into_iter()
            .next()
            .ok_or_else(|| no_page("The browser has no open page"));
    }
    let mut visible = Vec::new();
    for target in candidates {
        let session = link.session(&target.target_id, deadline)?;
        if page_visibility(&mut link.connection, &session, deadline)? == "visible" {
            visible.push(target);
        }
    }
    match visible.len() {
        1 => visible
            .into_iter()
            .next()
            .ok_or_else(|| no_page("No visible tab")),
        0 => Err(no_page("No tab in the browser window is visible")
            .with_suggestion("Restore the window if it is minimized, or open the page with `open-url --new-tab` and read it by its target")),
        _ => Err(AdapterError::new(
            ErrorCode::AmbiguousTarget,
            "More than one browser window shows a visible tab",
        )
        .with_suggestion("Name the window with --window-id from list-windows")
        .with_details(json!({
            "kind": "cdp_page_ambiguous",
            "targets": visible.iter().map(|target| &target.target_id).collect::<Vec<_>>(),
        }))
        .with_disposition(DeliverySemantics::not_delivered())),
    }
}

fn in_window(
    placed: Vec<(PageTarget, BrowserWindow)>,
    window: Option<&WindowInfo>,
) -> Result<Vec<PageTarget>, AdapterError> {
    let mut window_ids: Vec<i64> = placed.iter().map(|(_, window)| window.window_id).collect();
    window_ids.sort_unstable();
    window_ids.dedup();
    let bounds = window.and_then(|window| window.bounds);
    let Some(bounds) = bounds.filter(|_| window_ids.len() > 1) else {
        return Ok(placed.into_iter().map(|(target, _)| target).collect());
    };
    let matching: Vec<PageTarget> = placed
        .into_iter()
        .filter(|(_, browser_window)| browser_window.matches_bounds(&bounds, BOUNDS_TOLERANCE))
        .map(|(target, _)| target)
        .collect();
    if matching.is_empty() {
        return Err(
            no_page("No browser window matches the platform window's bounds")
                .with_suggestion("Run list-windows and pass the browser window's --window-id")
                .with_details(json!({
                    "kind": "cdp_window_unmatched",
                    "window_id": window.map(|window| window.id.clone()),
                })),
        );
    }
    Ok(matching)
}

fn no_page(message: &str) -> AdapterError {
    AdapterError::new(ErrorCode::WindowNotFound, message)
        .with_details(json!({ "kind": "cdp_page_not_found" }))
        .with_disposition(DeliverySemantics::not_delivered())
}

#[cfg(test)]
#[path = "page_choice_tests.rs"]
mod tests;
