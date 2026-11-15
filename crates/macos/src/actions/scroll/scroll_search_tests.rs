use super::{ScrollSignals, area_moved, send_steps, settle};
use agent_desktop_core::{AdapterError, ErrorCode, ScrollPage};

fn signals(bar: u64) -> ScrollSignals {
    ScrollSignals {
        bar: Some(Some(bar)),
        content: Some(Some(7)),
        descendant: None,
        sampled: 2,
    }
}

#[test]
fn a_signal_that_moves_settles_the_page_as_moved() {
    let mut reads = vec![signals(2), signals(1)].into_iter();
    let page = settle(signals(1), || reads.next().unwrap_or(signals(1)), || true);
    assert_eq!(page, ScrollPage::Moved);
}

#[test]
fn signals_unchanged_through_every_read_are_the_end_of_the_area() {
    let page = settle(signals(1), || signals(1), || true);
    assert_eq!(page, ScrollPage::AtEnd { delivered: true });
}

#[test]
fn unreadable_signals_leave_the_page_unconfirmed() {
    let unreadable = ScrollSignals {
        bar: Some(None),
        content: Some(None),
        descendant: None,
        sampled: 0,
    };
    let page = settle(signals(1), || unreadable, || true);
    assert_eq!(page, ScrollPage::Unconfirmed);
}

#[test]
fn a_spent_deadline_ends_the_reads_without_claiming_an_end() {
    let unreadable = ScrollSignals {
        bar: Some(None),
        content: None,
        descendant: None,
        sampled: 0,
    };
    let page = settle(signals(1), || unreadable, || false);
    assert_eq!(page, ScrollPage::Unconfirmed);
}

fn unanswered() -> AdapterError {
    AdapterError::new(ErrorCode::AppUnresponsive, "kAXErrorCannotComplete")
}

#[test]
fn the_first_step_the_area_takes_is_the_only_one_sent() {
    let mut sent = Vec::new();
    let page = {
        let mut part = || {
            sent.push("part");
            Ok(false)
        };
        let mut action = || Ok(true);
        let mut line = || -> Result<bool, AdapterError> { panic!("a later step ran") };
        send_steps(&mut [&mut part, &mut action, &mut line], true, || {
            ScrollPage::Moved
        })
    };
    assert_eq!(page.unwrap(), ScrollPage::Moved);
    assert_eq!(sent, vec!["part"]);
}

#[test]
fn a_refused_step_that_moved_nothing_passes_to_the_next() {
    let mut settles = vec![ScrollPage::Moved, ScrollPage::AtEnd { delivered: true }]
        .into_iter()
        .rev();
    let mut refused = || Err(unanswered());
    let mut next = || Ok(true);
    let page = send_steps(&mut [&mut refused, &mut next], true, || {
        settles.next().unwrap()
    });
    assert_eq!(page.unwrap(), ScrollPage::Moved);
}

#[test]
fn a_refused_step_that_moved_the_area_is_a_page() {
    let mut refused = || Err(unanswered());
    let mut after = || -> Result<bool, AdapterError> { panic!("ran after the page moved") };
    let page = send_steps(&mut [&mut refused, &mut after], true, || ScrollPage::Moved);
    assert_eq!(page.unwrap(), ScrollPage::Moved);
}

#[test]
fn a_refusal_the_reads_cannot_decide_ends_the_page_with_its_error() {
    let mut refused = || Err(unanswered());
    let mut after = || -> Result<bool, AdapterError> { panic!("ran after an undecided refusal") };
    let error = send_steps(&mut [&mut refused, &mut after], true, || {
        ScrollPage::Unconfirmed
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::AppUnresponsive);
}

#[test]
fn an_area_with_no_step_sends_nothing() {
    let mut none = || Ok(false);
    let error = send_steps(&mut [&mut none], true, || panic!("nothing was sent")).unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    assert_eq!(error.details.unwrap()["kind"], "scroll_search_no_page_step");
}

#[test]
fn a_sent_step_that_moved_nothing_while_the_bar_has_room_passes_to_the_next() {
    let mut settles = vec![ScrollPage::AtEnd { delivered: true }, ScrollPage::Moved].into_iter();
    let mut inert = || Ok(true);
    let mut next = || Ok(true);
    let page = send_steps(&mut [&mut inert, &mut next], true, || {
        settles.next().unwrap()
    });
    assert_eq!(page.unwrap(), ScrollPage::Moved);
}

#[test]
fn a_sent_step_that_moved_nothing_without_a_readable_bar_is_the_end() {
    let mut sent = || Ok(true);
    let mut after = || -> Result<bool, AdapterError> { panic!("ran past the end") };
    let page = send_steps(&mut [&mut sent, &mut after], false, || ScrollPage::AtEnd {
        delivered: true,
    });
    assert_eq!(page.unwrap(), ScrollPage::AtEnd { delivered: true });
}

#[test]
fn every_step_inert_while_the_bar_has_room_is_an_error_not_an_end() {
    let mut inert = || Ok(true);
    let error = send_steps(&mut [&mut inert], true, || ScrollPage::AtEnd {
        delivered: true,
    })
    .unwrap_err();
    assert_eq!(error.details.unwrap()["kind"], "scroll_search_inert");
}

#[test]
fn frames_that_moved_under_an_unmoved_bar_are_an_enclosing_scroll_not_a_page() {
    let before = signals(1);
    let after = ScrollSignals {
        content: Some(Some(8)),
        ..signals(1)
    };
    assert_eq!(area_moved(before, after), Some(false));
    assert_eq!(area_moved(before, signals(2)), Some(true));
}

#[test]
fn without_a_readable_bar_the_frames_decide() {
    let frames = |hash| ScrollSignals {
        bar: None,
        content: Some(Some(hash)),
        descendant: Some(Some(hash)),
        sampled: 4,
    };
    assert_eq!(area_moved(frames(1), frames(2)), Some(true));
    assert_eq!(area_moved(frames(1), frames(1)), Some(false));
}
