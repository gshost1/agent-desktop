//! A find that pages a scroll area until a lazily loaded list holds a match.
//!
//! A list that loads rows only near its viewport does not expose the rows it
//! has not loaded, so no single observation can find them. This pages the
//! root's scroll area one page at a time without physical input, looks again
//! after every page, and stops at the first match, at both ends of the area,
//! at the page cap or at the deadline. The view stays where the search
//! stopped and the response says so.

use super::{FindArgs, find_scroll_report::ScrollSearchReport, live};
use crate::{
    AdapterError, AppError, Deadline, DeliverySemantics, Direction, LocatorQuery, ScrollPage,
    adapter::{NativeHandle, PlatformAdapter},
    context::CommandContext,
};
use serde_json::{Value, json};
use std::time::Duration;

const MAX_SCROLL_PAGES: u32 = 40;
const SCROLL_SEARCH_TIMEOUT: Duration = Duration::from_secs(20);
const LOOK_BUDGET: Duration = Duration::from_secs(5);

/// What the search reads from and does to one scroll area.
pub(super) trait ScrollSearch {
    fn look(&mut self) -> Result<bool, AppError>;
    fn page(&mut self, direction: &Direction) -> Result<ScrollPage, AdapterError>;
}

pub(super) fn search(
    area: &mut impl ScrollSearch,
    first: Direction,
    deadline: Deadline,
) -> Result<ScrollSearchReport, AppError> {
    let mut report = ScrollSearchReport {
        stopped: "found",
        ..ScrollSearchReport::default()
    };
    if area.look()? {
        return Ok(report);
    }
    let second = opposite(&first);
    for direction in [first, second] {
        report.directions.push(direction_name(&direction));
        loop {
            if report.pages >= MAX_SCROLL_PAGES {
                report.stopped = "page_cap";
                return Ok(report);
            }
            if deadline.is_expired() {
                return Err(stopped(deadline.timeout_error().into(), &report));
            }
            match area.page(&direction) {
                Ok(ScrollPage::Moved) => {
                    report.pages += 1;
                    report.view_moved = true;
                }
                Ok(ScrollPage::Unconfirmed) => {
                    report.pages += 1;
                    report.unconfirmed_pages += 1;
                }
                Ok(ScrollPage::AtEnd { .. }) => break,
                Err(error) => return Err(stopped(error.into(), &report)),
            }
            if area.look().map_err(|error| stopped(error, &report))? {
                return Ok(report);
            }
        }
    }
    report.stopped = "both_ends";
    Ok(report)
}

/// An error that ends the search after it paged carries how far it got, and
/// cannot claim nothing was sent.
fn stopped(error: AppError, report: &ScrollSearchReport) -> AppError {
    let AppError::Adapter(error) = error else {
        return error;
    };
    let mut error = error.with_detail("scroll_search", json!(report));
    if report.may_have_moved()
        && matches!(
            error.disposition,
            DeliverySemantics::NotDelivered | DeliverySemantics::Unknown
        )
    {
        error = error.with_disposition(DeliverySemantics::delivered_unverified());
    }
    AppError::Adapter(error)
}

fn opposite(direction: &Direction) -> Direction {
    match direction {
        Direction::Up => Direction::Down,
        Direction::Down => Direction::Up,
        Direction::Left => Direction::Right,
        Direction::Right => Direction::Left,
    }
}

fn direction_name(direction: &Direction) -> &'static str {
    match direction {
        Direction::Up => "up",
        Direction::Down => "down",
        Direction::Left => "left",
        Direction::Right => "right",
    }
}

struct LiveScrollSearch<'a> {
    find: (&'a FindArgs, &'a LocatorQuery),
    env: (&'a dyn PlatformAdapter, &'a CommandContext),
    handle: NativeHandle,
    deadline: Deadline,
    last: Option<Value>,
}

impl ScrollSearch for LiveScrollSearch<'_> {
    fn look(&mut self) -> Result<bool, AppError> {
        let (args, query) = self.find;
        let budget = self.deadline.capped(LOOK_BUDGET);
        let (response, matched) = live::execute_within(args, query, self.env, budget)?;
        self.last = Some(response);
        Ok(matched)
    }

    fn page(&mut self, direction: &Direction) -> Result<ScrollPage, AdapterError> {
        let (adapter, _) = self.env;
        let lease = crate::ref_action::ref_action_wait::acquire_lease(adapter, self.deadline)?;
        adapter.scroll_page(&self.handle, direction, &lease)
    }
}

pub(super) fn execute(
    args: &FindArgs,
    query: &LocatorQuery,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let (direction, root_ref) = scroll_target(args)?;
    let deadline = match args.timeout_ms {
        Some(timeout_ms) => Deadline::after(timeout_ms)?,
        None => Deadline::from_duration(SCROLL_SEARCH_TIMEOUT)?,
    };
    let entry = crate::commands::helpers::load_ref_entry(root_ref, context)?;
    let handle =
        crate::commands::helpers::resolve_handle_within_deadline(adapter, &entry, deadline, None)?;
    let mut live = LiveScrollSearch {
        find: (args, query),
        env: (adapter, context),
        handle,
        deadline,
        last: None,
    };
    let report = search(&mut live, direction, deadline)?;
    let mut response = live.last.unwrap_or_else(|| json!({}));
    if let Some(object) = response.as_object_mut() {
        object.insert("scroll".into(), json!(report));
    }
    Ok(response)
}

fn scroll_target(args: &FindArgs) -> Result<(Direction, &str), AppError> {
    let direction = args.scope.scroll.clone().unwrap_or(Direction::Down);
    let Some(root_ref) = args.scope.root.as_deref() else {
        return Err(AppError::invalid_input_with_suggestion(
            "find --scroll needs --root",
            "Pass --root with the ref of the list or scroll area to page through, from a snapshot.",
        ));
    };
    if args.selection.count {
        return Err(AppError::invalid_input_with_suggestion(
            "find --scroll stops at the first page that holds a match, so it cannot --count",
            "Drop --count, or count without --scroll.",
        ));
    }
    Ok((direction, root_ref))
}

#[cfg(test)]
#[path = "find_scroll_tests.rs"]
mod tests;
