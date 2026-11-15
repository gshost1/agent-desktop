use super::*;
use crate::ErrorCode;

/// Replays one page outcome per call and reports a match once the list
/// has been paged `found_after` times.
struct ScriptedList {
    pages: Vec<Result<ScrollPage, AdapterError>>,
    sent: Vec<&'static str>,
    found_after: Option<usize>,
}

impl ScriptedList {
    fn new(pages: Vec<Result<ScrollPage, AdapterError>>, found_after: Option<usize>) -> Self {
        Self {
            pages: pages.into_iter().rev().collect(),
            sent: Vec::new(),
            found_after,
        }
    }
}

impl ScrollSearch for ScriptedList {
    fn look(&mut self) -> Result<bool, AppError> {
        Ok(self.found_after == Some(self.sent.len()))
    }

    fn page(&mut self, direction: &Direction) -> Result<ScrollPage, AdapterError> {
        self.sent.push(direction_name(direction));
        self.pages
            .pop()
            .unwrap_or(Ok(ScrollPage::AtEnd { delivered: false }))
    }
}

fn deadline() -> Deadline {
    Deadline::after(10_000).unwrap()
}

#[test]
fn a_match_already_loaded_is_returned_without_scrolling() {
    let mut list = ScriptedList::new(vec![], Some(0));
    let report = search(&mut list, Direction::Down, deadline()).unwrap();
    assert_eq!(report.stopped, "found");
    assert_eq!(report.pages, 0);
    assert!(!report.view_moved);
    assert!(list.sent.is_empty());
}

#[test]
fn the_search_pages_until_a_match_loads_and_says_the_view_moved() {
    let mut list = ScriptedList::new(vec![Ok(ScrollPage::Moved); 3], Some(3));
    let report = search(&mut list, Direction::Down, deadline()).unwrap();
    assert_eq!(report.stopped, "found");
    assert_eq!(report.pages, 3);
    assert!(report.view_moved);
    assert_eq!(list.sent, ["down"; 3]);
}

#[test]
fn reaching_the_end_turns_back_and_searches_the_other_way() {
    let pages = vec![
        Ok(ScrollPage::Moved),
        Ok(ScrollPage::AtEnd { delivered: true }),
        Ok(ScrollPage::Moved),
        Ok(ScrollPage::Moved),
    ];
    let mut list = ScriptedList::new(pages, Some(4));
    let report = search(&mut list, Direction::Down, deadline()).unwrap();
    assert_eq!(report.stopped, "found");
    assert_eq!(report.directions, ["down", "up"]);
    assert_eq!(list.sent, ["down", "down", "up", "up"]);
}

#[test]
fn both_ends_without_a_match_is_a_clear_not_found() {
    let mut list = ScriptedList::new(vec![Ok(ScrollPage::Moved)], None);
    let report = search(&mut list, Direction::Down, deadline()).unwrap();
    assert_eq!(report.stopped, "both_ends");
    assert_eq!(report.pages, 1);
    assert!(report.view_moved);
}

#[test]
fn the_page_cap_ends_an_endless_list() {
    let mut list = ScriptedList::new(vec![Ok(ScrollPage::Moved); 100], None);
    let report = search(&mut list, Direction::Down, deadline()).unwrap();
    assert_eq!(report.stopped, "page_cap");
    assert_eq!(report.pages, MAX_SCROLL_PAGES);
}

#[test]
fn an_unconfirmed_page_is_counted_and_reported() {
    let mut list = ScriptedList::new(vec![Ok(ScrollPage::Unconfirmed)], Some(1));
    let report = search(&mut list, Direction::Down, deadline()).unwrap();
    assert_eq!(report.unconfirmed_pages, 1);
    assert!(!report.view_moved);
    assert!(report.may_have_moved());
}

#[test]
fn a_failure_after_the_view_moved_cannot_claim_nothing_was_sent() {
    let refusal = AdapterError::new(ErrorCode::ActionFailed, "page refused")
        .with_disposition(DeliverySemantics::not_delivered());
    let mut list = ScriptedList::new(vec![Ok(ScrollPage::Moved), Err(refusal)], None);
    let Err(AppError::Adapter(error)) = search(&mut list, Direction::Down, deadline()) else {
        panic!("the refusal ends the search");
    };
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.expect("details");
    assert_eq!(details["scroll_search"]["pages"], 1);
    assert_eq!(details["scroll_search"]["view_moved"], true);
}

#[test]
fn a_refusal_before_any_page_moved_stays_not_delivered() {
    let refusal = AdapterError::not_supported("scroll_page");
    let mut list = ScriptedList::new(vec![Err(refusal)], None);
    let Err(AppError::Adapter(error)) = search(&mut list, Direction::Down, deadline()) else {
        panic!("an unsupported page ends the search");
    };
    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    assert_ne!(error.disposition, DeliverySemantics::delivered_unverified());
}
