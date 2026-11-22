use std::cell::{Cell, RefCell};

use super::*;
use crate::system::launch::launch_observe::tests::app;

fn child(path: &str) -> ChildAddress {
    (":1.5".to_owned(), path.to_owned())
}

#[test]
fn a_root_without_children_needs_no_full_read() {
    let full = Cell::new(0);
    let mut probe = WindowProbe::new(
        |_: &Running| Some(Vec::new()),
        |_: &Running| {
            full.set(full.get() + 1);
            true
        },
    );
    for _ in 0..5 {
        assert!(!probe.has_window(&app(200, "tray")));
    }
    assert_eq!(full.get(), 0);
}

#[test]
fn a_seen_window_is_remembered_for_the_process_instance() {
    let listed = Cell::new(0);
    let full = Cell::new(0);
    let mut probe = WindowProbe::new(
        |_: &Running| {
            listed.set(listed.get() + 1);
            Some(vec![child("/frame")])
        },
        |_: &Running| {
            full.set(full.get() + 1);
            true
        },
    );
    let windowed = app(200, "editor");
    assert!(probe.has_window(&windowed));
    assert!(probe.has_window(&windowed));
    assert_eq!((listed.get(), full.get()), (1, 1));
    let mut replacement = windowed.clone();
    replacement.instance = "boot:replacement".into();
    assert!(probe.has_window(&replacement));
    assert_eq!(full.get(), 2);
}

#[test]
fn unchanged_children_reuse_a_windowless_full_read() {
    let children = RefCell::new(vec![child("/hidden")]);
    let full = Cell::new(0);
    let mut probe = WindowProbe::new(
        |_: &Running| Some(children.borrow().clone()),
        |_: &Running| {
            full.set(full.get() + 1);
            false
        },
    );
    let tray = app(200, "tray");
    for _ in 0..4 {
        assert!(!probe.has_window(&tray));
    }
    assert_eq!(full.get(), 1);
    children.borrow_mut().push(child("/frame"));
    assert!(!probe.has_window(&tray));
    assert_eq!(full.get(), 2);
}

#[test]
fn a_reused_windowless_read_expires() {
    let full = Cell::new(0);
    let mut probe = WindowProbe::new(
        |_: &Running| Some(vec![child("/hidden")]),
        |_: &Running| {
            full.set(full.get() + 1);
            full.get() > 1
        },
    );
    let app = app(200, "late");
    assert!(!probe.has_window(&app));
    std::thread::sleep(FULL_READ_REUSE);
    assert!(probe.has_window(&app));
    assert_eq!(full.get(), 2);
}

#[test]
fn an_unreadable_child_list_falls_back_to_the_full_read() {
    let full = Cell::new(0);
    let mut probe = WindowProbe::new(
        |_: &Running| None,
        |_: &Running| {
            full.set(full.get() + 1);
            false
        },
    );
    let app = app(200, "slow");
    assert!(!probe.has_window(&app));
    assert!(!probe.has_window(&app));
    assert_eq!(full.get(), 2);
}
