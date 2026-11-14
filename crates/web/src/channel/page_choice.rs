use std::time::Duration;

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode, WindowInfo};
use serde_json::json;

use super::browser_link::BrowserLink;
use crate::cdp::protocol_error::error_kind;
use crate::{BrowserWindow, PageTarget, list_page_targets, page_visibility};

#[path = "bounds_fit.rs"]
mod bounds_fit;
#[path = "window_title.rs"]
mod window_title;

use bounds_fit::BoundsFit;
use window_title::shows_tab_title;

const TAB_READ_BUDGET: Duration = Duration::from_millis(1_000);
const TRANSPORT_CLOSED: &str = "cdp_transport_closed";

type Placed = (PageTarget, BrowserWindow);

/// The page target a platform window shows. Targets are grouped by the
/// browser window that holds them, and the groups whose bounds fit the
/// platform window's bounds most closely are kept, at any display scale
/// (all groups when the browser has one window or the platform window
/// reports no bounds). A group that fits by size alone must also hold a tab
/// whose title the platform window carries. Among the kept tabs, the one
/// whose title the platform window carries wins, since a browser titles each
/// window after its active tab even while the window is covered. Only when
/// the title leaves more than one tab is each tab's
/// `document.visibilityState` read, and a tab whose read fails is skipped.
pub fn choose_page(
    link: &mut BrowserLink,
    window: Option<&WindowInfo>,
    deadline: Deadline,
) -> Result<PageTarget, AdapterError> {
    let targets = list_page_targets(&mut link.connection, deadline)?;
    if targets.is_empty() {
        return Err(no_page_target());
    }
    let mut placed = Vec::new();
    for target in targets {
        if let Ok(browser_window) =
            BrowserWindow::of_target(&mut link.connection, &target.target_id, deadline)
        {
            placed.push((target, browser_window));
        }
    }
    if placed.is_empty() {
        return Err(no_page("No page of the browser reports its window"));
    }
    let (candidates, fit) = in_window(placed, window)?;
    let pool = match titled(&candidates, window) {
        Some(titled) => titled,
        None if fit == Some(BoundsFit::Sized) => return Err(unmatched(window)),
        None => candidates,
    };
    if pool.len() == 1 {
        return only(pool);
    }
    let visible = visible_tabs(link, &pool, deadline)?;
    if visible.len() == 1 {
        return only(visible);
    }
    Err(undecided(&pool, &visible, window))
}

fn in_window(
    placed: Vec<Placed>,
    window: Option<&WindowInfo>,
) -> Result<(Vec<Placed>, Option<BoundsFit>), AdapterError> {
    let bounds = window.and_then(|window| window.bounds);
    let Some(bounds) = bounds.filter(|_| browser_window_ids(&placed).len() > 1) else {
        return Ok((placed, None));
    };
    let fitted: Vec<(Placed, BoundsFit)> = placed
        .into_iter()
        .filter_map(|placed| BoundsFit::of(&placed.1.bounds, &bounds).map(|fit| (placed, fit)))
        .collect();
    let Some(closest) = fitted.iter().map(|(_, fit)| *fit).min() else {
        return Err(unmatched(window));
    };
    let closest_fitted = fitted
        .into_iter()
        .filter(|(_, fit)| *fit == closest)
        .map(|(placed, _)| placed)
        .collect();
    Ok((closest_fitted, Some(closest)))
}

fn unmatched(window: Option<&WindowInfo>) -> AdapterError {
    no_page("No browser window matches the platform window's bounds and title")
        .with_suggestion(
            "The window may show no page, such as a dialog; pass the --window-id of a browser \
             window from list-windows",
        )
        .with_details(json!({
            "kind": "cdp_window_unmatched",
            "window_id": window.map(|window| window.id.clone()),
        }))
}

/// The candidates whose title the window carries, keeping only the longest
/// such title, or `None` when the window carries no candidate's title.
fn titled(candidates: &[Placed], window: Option<&WindowInfo>) -> Option<Vec<Placed>> {
    let window_title = window?.title.as_str();
    let shown_length = |target: &PageTarget| {
        shows_tab_title(window_title, &target.title).then(|| target.title.trim().len())
    };
    let longest = candidates
        .iter()
        .filter_map(|(target, _)| shown_length(target))
        .max()?;
    Some(
        candidates
            .iter()
            .filter(|(target, _)| shown_length(target) == Some(longest))
            .cloned()
            .collect(),
    )
}

fn visible_tabs(
    link: &mut BrowserLink,
    pool: &[Placed],
    deadline: Deadline,
) -> Result<Vec<Placed>, AdapterError> {
    let mut visible = Vec::new();
    for placed in pool {
        match tab_visibility(link, &placed.0.target_id, deadline.capped(TAB_READ_BUDGET)) {
            Ok(state) if state == "visible" => visible.push(placed.clone()),
            Ok(_) => {}
            Err(error) if error_kind(&error) == Some(TRANSPORT_CLOSED) => return Err(error),
            Err(_) => deadline.ensure_remaining()?,
        }
    }
    Ok(visible)
}

fn tab_visibility(
    link: &mut BrowserLink,
    target_id: &str,
    deadline: Deadline,
) -> Result<String, AdapterError> {
    let session = link.session(target_id, deadline)?;
    page_visibility(&mut link.connection, &session, deadline)
}

fn undecided(pool: &[Placed], visible: &[Placed], window: Option<&WindowInfo>) -> AdapterError {
    let shown = if visible.is_empty() { pool } else { visible };
    let details = |kind: &str| {
        json!({
            "kind": kind,
            "targets": shown.iter().map(|(target, _)| &target.target_id).collect::<Vec<_>>(),
            "browser_window_ids": browser_window_ids(shown),
        })
    };
    if window.is_none() && !visible.is_empty() {
        return ambiguous("More than one browser window shows a visible tab")
            .with_suggestion("Name the window with --window-id from list-windows")
            .with_details(details("cdp_page_ambiguous"));
    }
    if let Some(window) = window.filter(|_| browser_window_ids(shown).len() > 1) {
        return ambiguous(
            "Several browser windows could be this window, and neither its title nor page \
             visibility tells them apart",
        )
        .with_suggestion(
            "Bring the window to the front, or move one of the browser windows so their frames \
             differ, then retry",
        )
        .with_details(details("cdp_window_indistinct"))
        .with_platform_detail(format!("window {} titled '{}'", window.id, window.title));
    }
    if !visible.is_empty() {
        return ambiguous("More than one tab of the browser window is visible")
            .with_suggestion("Select the tab to read in the browser, then retry")
            .with_details(details("cdp_page_ambiguous"));
    }
    no_page("No tab in the browser window is visible or carries the window's title")
        .with_suggestion("Bring the window to the front or restore it if minimized, then retry")
        .with_details(details("cdp_page_not_found"))
}

fn browser_window_ids(placed: &[Placed]) -> Vec<i64> {
    let mut ids: Vec<i64> = placed.iter().map(|(_, window)| window.window_id).collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

fn only(pool: Vec<Placed>) -> Result<PageTarget, AdapterError> {
    pool.into_iter()
        .next()
        .map(|(target, _)| target)
        .ok_or_else(|| no_page("The browser has no open page"))
}

fn ambiguous(message: &str) -> AdapterError {
    AdapterError::new(ErrorCode::AmbiguousTarget, message)
        .with_disposition(DeliverySemantics::not_delivered())
}

/// The error for a browser with no page target at all, such as a fresh
/// profile that shows only its first-run dialog.
pub fn no_page_target() -> AdapterError {
    AdapterError::new(ErrorCode::WindowNotFound, "The browser has no open page")
        .with_suggestion("Load one with `open-url <url> --app <browser>`, then snapshot again")
        .with_details(json!({ "kind": NO_PAGE_TARGET }))
        .with_disposition(DeliverySemantics::not_delivered())
}

/// The `details.kind` of [`no_page_target`].
pub const NO_PAGE_TARGET: &str = "cdp_browser_has_no_page";

fn no_page(message: &str) -> AdapterError {
    AdapterError::new(ErrorCode::WindowNotFound, message)
        .with_details(json!({ "kind": "cdp_page_not_found" }))
        .with_disposition(DeliverySemantics::not_delivered())
}

#[cfg(test)]
#[path = "page_choice_tests.rs"]
mod tests;
