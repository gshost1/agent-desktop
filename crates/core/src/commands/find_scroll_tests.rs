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
    let endless = vec![Ok(ScrollPage::Moved); MAX_SCROLL_PAGES as usize + 10];
    let mut list = ScriptedList::new(endless, None);
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
    assert_eq!(details["scroll_search"]["stopped"], "error");
}

#[test]
fn a_spent_deadline_ends_the_search_and_says_so() {
    let spent = Deadline::after(1).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));
    let mut list = ScriptedList::new(vec![Ok(ScrollPage::Moved)], None);
    let Err(AppError::Adapter(error)) = search(&mut list, Direction::Down, spent) else {
        panic!("a spent deadline ends the search");
    };
    assert_eq!(error.code, ErrorCode::Timeout);
    assert!(list.sent.is_empty());
    assert_eq!(
        error.details.expect("details")["scroll_search"]["stopped"],
        "deadline"
    );
}

#[test]
fn a_refusal_before_any_page_moved_stays_not_delivered() {
    let refusal = AdapterError::not_supported("scroll_page");
    let mut list = ScriptedList::new(vec![Err(refusal)], None);
    let Err(AppError::Adapter(error)) = search(&mut list, Direction::Down, deadline()) else {
        panic!("an unsupported page ends the search");
    };
    assert_eq!(error.code, ErrorCode::PlatformNotSupported);
    assert_ne!(error.disposition, DeliverySemantics::delivered_unverified());
}

#[test]
fn a_look_that_runs_out_of_time_is_reported_as_the_deadline() {
    struct SlowLook(bool);
    impl ScrollSearch for SlowLook {
        fn look(&mut self) -> Result<bool, AppError> {
            if std::mem::replace(&mut self.0, true) {
                return Err(AdapterError::new(ErrorCode::Timeout, "look ran out").into());
            }
            Ok(false)
        }
        fn page(&mut self, _: &Direction) -> Result<ScrollPage, AdapterError> {
            Ok(ScrollPage::Moved)
        }
    }
    let Err(AppError::Adapter(error)) = search(&mut SlowLook(false), Direction::Down, deadline())
    else {
        panic!("the timed-out look ends the search");
    };
    let details = error.details.expect("details");
    assert_eq!(details["scroll_search"]["stopped"], "deadline");
    assert_eq!(details["scroll_search"]["pages"], 1);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}

fn scroll_args(selection: crate::commands::find::FindSelectionArgs) -> FindArgs {
    FindArgs {
        scope: crate::commands::find::FindScopeArgs {
            app: None,
            window_id: None,
            root: Some("@s1:e1".into()),
            surface: crate::SnapshotSurface::Window,
            scroll: Some(Direction::Down),
        },
        filter: crate::commands::find::FindFilterArgs {
            role: None,
            name: Some("Row 300".into()),
            description: None,
            native_id: None,
            value: None,
            text: None,
            exact: true,
        },
        states: Vec::new(),
        timeout_ms: None,
        selection,
    }
}

fn selection() -> crate::commands::find::FindSelectionArgs {
    crate::commands::find::FindSelectionArgs {
        count: false,
        first: false,
        last: false,
        nth: None,
        limit: None,
        allow_partial: false,
    }
}

#[test]
fn a_selection_that_needs_the_whole_list_is_refused_before_any_page() {
    let refusals = [
        (
            "count",
            crate::commands::find::FindSelectionArgs {
                count: true,
                ..selection()
            },
        ),
        (
            "last",
            crate::commands::find::FindSelectionArgs {
                last: true,
                ..selection()
            },
        ),
        (
            "nth",
            crate::commands::find::FindSelectionArgs {
                nth: Some(20),
                ..selection()
            },
        ),
        (
            "allow_partial",
            crate::commands::find::FindSelectionArgs {
                allow_partial: true,
                ..selection()
            },
        ),
    ];
    for (flag, refused) in refusals {
        let Err(error) = scroll_target(&scroll_args(refused)) else {
            panic!("--scroll with {flag} must be refused");
        };
        assert_eq!(error.code(), ErrorCode::InvalidArgs.as_str(), "{flag}");
    }
    let first = crate::commands::find::FindSelectionArgs {
        first: true,
        ..selection()
    };
    assert!(scroll_target(&scroll_args(first)).is_ok());
    let limited = crate::commands::find::FindSelectionArgs {
        limit: Some(3),
        ..selection()
    };
    assert!(scroll_target(&scroll_args(limited)).is_ok());
}

#[test]
fn only_a_returned_element_counts_as_found() {
    assert!(holds_a_match(&json!({ "match": { "ref_id": "@s1:e4" } })));
    assert!(holds_a_match(
        &json!({ "matches": [{ "ref_id": "@s1:e4" }] })
    ));
    assert!(!holds_a_match(&json!({ "match": null })));
    assert!(!holds_a_match(&json!({ "matches": [] })));
    assert!(!holds_a_match(&json!({ "count": 12 })));
}

#[test]
fn a_page_sent_at_the_end_makes_a_later_failure_unsafe_to_retry() {
    let expired =
        AdapterError::timeout("deadline").with_disposition(DeliverySemantics::not_delivered());
    let pages = vec![Ok(ScrollPage::AtEnd { delivered: true }), Err(expired)];
    let mut list = ScriptedList::new(pages, None);
    let Err(AppError::Adapter(error)) = search(&mut list, Direction::Down, deadline()) else {
        panic!("the failure ends the search");
    };
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.expect("details");
    assert_eq!(details["scroll_search"]["pages"], 0);
    assert_eq!(details["scroll_search"]["pages_at_end"], 1);
}

#[test]
fn a_page_at_the_end_that_sent_nothing_stays_retry_safe() {
    let refused = AdapterError::new(ErrorCode::ActionFailed, "refused")
        .with_disposition(DeliverySemantics::not_delivered());
    let pages = vec![Ok(ScrollPage::AtEnd { delivered: false }), Err(refused)];
    let mut list = ScriptedList::new(pages, None);
    let Err(AppError::Adapter(error)) = search(&mut list, Direction::Down, deadline()) else {
        panic!("the failure ends the search");
    };
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}
