use super::*;
use crate::system::menu::menu_surface::MenuFamily;
use crate::tree::AtspiElement;
use crate::tree::menu::menu_context::MenuContext;
use agent_desktop_core::{ErrorCode, SnapshotSurface};
use std::time::Instant;

fn popup() -> MenuPopup {
    MenuPopup {
        root: AtspiElement::new(":1.2", "/menu", 7),
        family: MenuFamily::Qt,
        items: 1,
        context: MenuContext::test("/menu", 1),
    }
}

#[test]
fn a_first_read_success_is_not_polled_or_delayed() {
    let deadline = Deadline::after(5_000).unwrap();
    let mut reads = 0;
    let found = await_popup(deadline, |budget| {
        reads += 1;
        assert!(budget.remaining() > Duration::from_secs(1));
        Ok(vec![popup()])
    })
    .unwrap();
    assert_eq!(found.root.path(), "/menu");
    assert_eq!(reads, 1);
}

#[test]
fn a_menu_that_opens_after_the_first_read_is_retried() {
    let mut reads = 0;
    let started = Instant::now();
    let found = await_popup(Deadline::after(5_000).unwrap(), |budget| {
        reads += 1;
        if reads == 1 {
            Ok(Vec::new())
        } else {
            assert!(budget.remaining() <= OPENING_BUDGET);
            Ok(vec![popup()])
        }
    })
    .unwrap();
    assert_eq!(found.root.path(), "/menu");
    assert_eq!(reads, 2);
    assert!(started.elapsed() >= POLL_INTERVAL);
}

#[test]
fn an_absent_menu_is_bounded_and_keeps_the_surface_absent_error() {
    let started = Instant::now();
    let mut reads = 0;
    let result = await_popup(Deadline::after(5_000).unwrap(), |_| {
        reads += 1;
        Ok(Vec::new())
    });
    let error = result.err().unwrap();
    assert!(error.is_confirmed_surface_absence(SnapshotSurface::Menu));
    assert!(started.elapsed() >= OPENING_BUDGET);
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(reads > 1);
    assert!(reads <= 22);
}

#[test]
fn the_callers_shorter_deadline_bounds_the_opening_wait() {
    let started = Instant::now();
    let result = await_popup(Deadline::after(40).unwrap(), |_| Ok(Vec::new()));
    assert!(
        result
            .err()
            .unwrap()
            .is_confirmed_surface_absence(SnapshotSurface::Menu)
    );
    assert!(started.elapsed() < OPENING_BUDGET);
}

#[test]
fn inventory_errors_are_not_retried_or_reported_as_absence() {
    for fail_on in [1, 2] {
        let mut reads = 0;
        let result = await_popup(Deadline::after(5_000).unwrap(), |_| {
            reads += 1;
            if reads == fail_on {
                Err(AdapterError::timeout("incomplete menu inventory"))
            } else {
                Ok(Vec::new())
            }
        });
        assert_eq!(result.err().unwrap().code, ErrorCode::Timeout);
        assert_eq!(reads, fail_on);
    }
}

#[test]
fn expiry_during_a_retry_preserves_the_completed_initial_absence() {
    let mut reads = 0;
    let result = await_popup(Deadline::after(70).unwrap(), |budget| {
        reads += 1;
        if reads == 1 {
            return Ok(Vec::new());
        }
        std::thread::sleep(budget.remaining());
        Err(budget.timeout_error())
    });
    assert!(
        result
            .err()
            .unwrap()
            .is_confirmed_surface_absence(SnapshotSurface::Menu)
    );
    assert_eq!(reads, 2);
}
