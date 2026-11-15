use super::{POPUP_MENU_WINDOW_LEVEL, menu_numbers_from};
use crate::system::window::cg_window::WindowDictionary;
use core_foundation::{
    base::TCFType, dictionary::CFDictionary, number::CFNumber, string::CFString,
};

fn window(pid: i32, layer: i32, number: i32) -> WindowDictionary {
    CFDictionary::from_CFType_pairs(&[
        (
            CFString::new("kCGWindowLayer"),
            CFNumber::from(layer).as_CFType(),
        ),
        (
            CFString::new("kCGWindowOwnerPID"),
            CFNumber::from(pid).as_CFType(),
        ),
        (
            CFString::new("kCGWindowNumber"),
            CFNumber::from(number).as_CFType(),
        ),
    ])
}

#[test]
fn only_pop_up_menu_windows_of_the_target_process_are_read() {
    let windows = vec![window(41, 0, 1), window(41, 101, 2), window(52, 101, 4)];

    assert_eq!(POPUP_MENU_WINDOW_LEVEL, 101);
    assert_eq!(menu_numbers_from(&windows, 41), vec![2]);
    assert!(menu_numbers_from(&windows, 99).is_empty());
}

#[test]
fn floating_panels_tooltips_and_status_items_are_not_menus() {
    let windows = vec![window(41, 3, 5), window(41, 200, 6), window(41, 25, 7)];

    assert!(menu_numbers_from(&windows, 41).is_empty());
}

#[test]
fn a_window_without_a_layer_is_not_a_menu() {
    let bare = CFDictionary::from_CFType_pairs(&[
        (
            CFString::new("kCGWindowOwnerPID"),
            CFNumber::from(41).as_CFType(),
        ),
        (
            CFString::new("kCGWindowNumber"),
            CFNumber::from(9).as_CFType(),
        ),
    ]);

    assert!(menu_numbers_from(&[bare], 41).is_empty());
}

fn search_past_budget() -> agent_desktop_core::AdapterError {
    agent_desktop_core::AdapterError::new(
        agent_desktop_core::ErrorCode::AppUnresponsive,
        "Accessibility surface search exceeded its node budget",
    )
    .with_details(serde_json::json!({ "kind": "surface_search_limit", "limit": 2048 }))
}

fn other_failure() -> agent_desktop_core::AdapterError {
    agent_desktop_core::AdapterError::timeout("menu read did not finish")
}

#[test]
fn a_menu_window_on_screen_is_a_menu_without_asking_accessibility() {
    let shown = super::menu_shown_from(Ok(vec![12]), || panic!("the window server answered"));

    assert!(shown.unwrap());
}

#[test]
fn a_search_past_its_budget_is_no_menu_when_the_window_server_lists_none() {
    assert!(!super::menu_shown_from(Ok(Vec::new()), || Err(search_past_budget())).unwrap());
}

#[test]
fn a_search_past_its_budget_stays_an_error_when_the_window_server_failed() {
    let failed = Err(other_failure());

    assert!(super::menu_shown_from(failed, || Err(search_past_budget())).is_err());
}

#[test]
fn other_accessibility_answers_pass_through() {
    assert!(super::menu_shown_from(Ok(Vec::new()), || Ok(true)).unwrap());
    assert!(!super::menu_shown_from(Ok(Vec::new()), || Ok(false)).unwrap());
    assert!(super::menu_shown_from(Ok(Vec::new()), || Err(other_failure())).is_err());
}
