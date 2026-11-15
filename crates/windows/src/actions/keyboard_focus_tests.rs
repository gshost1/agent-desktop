use super::{KeyboardFocus, establish_with};
use agent_desktop_core::{AdapterError, Deadline, ErrorCode, StepMechanism};
use std::cell::Cell;
use std::time::{Duration, Instant};

fn deadline(ms: u64) -> Deadline {
    Deadline::after(ms).expect("deadline")
}

#[test]
fn a_target_already_holding_focus_is_not_refocused() {
    let asked = Cell::new(false);

    let focus = establish_with(
        deadline(5_000),
        || {
            asked.set(true);
            Ok(())
        },
        |_| Ok(true),
    )
    .expect("focus");

    assert_eq!(focus, KeyboardFocus::AlreadyFocused);
    assert!(!asked.get());
    assert!(focus.is_ready());
    assert!(focus.steps().is_empty());
}

#[test]
fn an_unfocused_target_is_focused_before_the_keystrokes() {
    let asked = Cell::new(false);
    let reads = Cell::new(0);

    let focus = establish_with(
        deadline(5_000),
        || {
            asked.set(true);
            Ok(())
        },
        |_| {
            reads.set(reads.get() + 1);
            Ok(asked.get() && reads.get() >= 3)
        },
    )
    .expect("focus");

    assert_eq!(focus, KeyboardFocus::FocusedBySetFocus);
    assert!(focus.is_ready());
    let steps = focus.steps();
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].label(), "UIElement.SetFocus");
    assert_eq!(steps[0].mechanism(), Some(StepMechanism::SemanticApi));
    assert_eq!(steps[0].verified(), Some(true));
}

#[test]
fn a_target_that_refuses_set_focus_stays_unfocused() {
    let focus = establish_with(
        deadline(5_000),
        || Err(AdapterError::internal("not focusable")),
        |_| Ok(false),
    )
    .expect("focus");

    assert_eq!(focus, KeyboardFocus::NotFocused);
    assert!(!focus.is_ready());
    assert!(focus.steps().is_empty());
}

#[test]
fn focus_that_never_lands_gives_up_after_the_settle_window() {
    let started = Instant::now();

    let focus = establish_with(deadline(10_000), || Ok(()), |_| Ok(false)).expect("focus");

    assert_eq!(focus, KeyboardFocus::NotFocused);
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn a_failed_focus_read_is_returned_not_swallowed() {
    let error = establish_with(
        deadline(5_000),
        || Ok(()),
        |_| Err(AdapterError::new(ErrorCode::StaleRef, "owner exited")),
    )
    .expect_err("read error");

    assert_eq!(error.code, ErrorCode::StaleRef);
}
