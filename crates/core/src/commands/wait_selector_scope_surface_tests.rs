use std::time::Duration;

use super::{
    MAIN, NEVER, OTHER, ScriptedApp, Window, adapter_error, entry, ms, open, wait_after_action_on,
};
use crate::wait::deadline::VirtualClock;
use crate::{ErrorCode, refs::refs_test_support::HomeGuard};

fn menubar_entry() -> crate::RefEntry {
    let mut entry = entry();
    entry.source.source_surface = crate::SnapshotSurface::Menubar;
    entry.source.source_window_id = None;
    entry.source.source_window_title = Some(String::new());
    entry
}

#[test]
fn a_menu_bar_item_wait_reads_the_focused_window_its_command_acts_in() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let app = ScriptedApp::new(vec![open(MAIN, 0), open(OTHER, 0)], (MAIN, ms(300)));

    let result = wait_after_action_on(&app, menubar_entry(), 10_000).unwrap();

    assert_eq!(result["window"]["id"], MAIN);
    assert_eq!(result["matched_selector"], ":Width");
    assert!(clock.elapsed() < ms(1_000), "{:?}", clock.elapsed());
}

#[test]
fn a_menu_bar_item_wait_ignores_an_unfocused_window_open_before_the_action() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let app = ScriptedApp::new(vec![open(MAIN, 0), open(OTHER, 0)], (OTHER, ms(0)));

    let error = adapter_error(wait_after_action_on(&app, menubar_entry(), 2_000));

    assert_eq!(error.code, ErrorCode::Timeout);
    assert!(clock.elapsed() >= ms(2_000));
}

#[test]
fn the_wait_reads_the_sheet_the_action_opened_on_its_window() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let mut app = ScriptedApp::new(vec![open(MAIN, 0)], (OTHER, NEVER));
    app.sheet = (MAIN, ms(300));

    let result = wait_after_action_on(&app, entry(), 10_000).unwrap();

    assert_eq!(result["window"]["id"], MAIN);
    assert_eq!(result["matched_selector"], ":Width");
    assert!(clock.elapsed() < ms(1_000), "{:?}", clock.elapsed());
}

#[test]
fn a_menu_bar_item_wait_reads_the_sheet_its_command_opened() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let main = Window {
        id: MAIN,
        opens: Duration::ZERO,
        closes: NEVER,
    };
    let mut app = ScriptedApp::new(vec![main, open(OTHER, 0)], (OTHER, NEVER));
    app.sheet = (MAIN, ms(300));

    let result = wait_after_action_on(&app, menubar_entry(), 10_000).unwrap();

    assert_eq!(result["window"]["id"], MAIN);
    assert!(clock.elapsed() < ms(1_000), "{:?}", clock.elapsed());
}

#[test]
fn a_sheet_that_never_opens_still_times_out() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let app = ScriptedApp::new(vec![open(MAIN, 0)], (OTHER, NEVER));

    let error = adapter_error(wait_after_action_on(&app, entry(), 2_000));

    assert_eq!(error.code, ErrorCode::Timeout);
    assert!(clock.elapsed() >= ms(2_000));
}
