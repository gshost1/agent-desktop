use super::{DOCK_WINDOW_LEVEL, front_foreign_window, stack_from};
use crate::system::window::cg_window::WindowDictionary;
use agent_desktop_core::{Point, Rect};
use core_foundation::{
    base::TCFType, dictionary::CFDictionary, number::CFNumber, string::CFString,
};

const TARGET: i32 = 41;
const OTHER: i32 = 52;
const TOOL: i32 = 63;

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn point() -> Point {
    Point { x: 150.0, y: 150.0 }
}

fn not_a_tool(_: i32) -> bool {
    false
}

fn whole_screen() -> Rect {
    rect(0.0, 0.0, 1470.0, 956.0)
}

#[test]
fn a_floating_panel_of_another_process_over_the_target_is_the_occluder() {
    let panel = rect(100.0, 100.0, 200.0, 200.0);
    let stack = vec![(OTHER, 3, panel), (TARGET, 0, whole_screen())];

    assert_eq!(
        front_foreign_window(&stack, &point(), TARGET, not_a_tool),
        Some(panel)
    );
}

#[test]
fn a_normal_window_of_another_process_in_front_is_the_occluder() {
    let window = rect(0.0, 0.0, 400.0, 400.0);
    let stack = vec![(OTHER, 0, window), (TARGET, 0, whole_screen())];

    assert_eq!(
        front_foreign_window(&stack, &point(), TARGET, not_a_tool),
        Some(window)
    );
}

#[test]
fn the_targets_own_window_in_front_gives_no_evidence() {
    let stack = vec![
        (TARGET, 0, rect(0.0, 0.0, 400.0, 400.0)),
        (OTHER, 0, whole_screen()),
    ];

    assert_eq!(
        front_foreign_window(&stack, &point(), TARGET, not_a_tool),
        None
    );
}

#[test]
fn dock_and_screenshot_chrome_over_the_whole_screen_is_passed_over() {
    let stack = vec![
        (OTHER, 24, whole_screen()),
        (OTHER, DOCK_WINDOW_LEVEL, whole_screen()),
        (TARGET, 0, rect(0.0, 0.0, 400.0, 400.0)),
    ];

    assert_eq!(
        front_foreign_window(&stack, &point(), TARGET, not_a_tool),
        None
    );
}

#[test]
fn a_target_window_above_the_dock_level_is_found_before_lower_windows() {
    let stack = vec![
        (TARGET, 25, rect(100.0, 0.0, 200.0, 300.0)),
        (OTHER, 0, whole_screen()),
    ];

    assert_eq!(
        front_foreign_window(&stack, &point(), TARGET, not_a_tool),
        None
    );
}

#[test]
fn agent_desktops_own_overlay_windows_are_passed_over() {
    let stack = vec![
        (TOOL, 3, rect(140.0, 140.0, 20.0, 20.0)),
        (TARGET, 0, whole_screen()),
    ];

    assert_eq!(
        front_foreign_window(&stack, &point(), TARGET, |pid| pid == TOOL),
        None
    );
}

#[test]
fn windows_that_do_not_contain_the_point_are_ignored() {
    let stack = vec![
        (OTHER, 3, rect(0.0, 0.0, 100.0, 100.0)),
        (OTHER, 0, rect(150.0, 150.0, 0.0, 50.0)),
        (TARGET, 0, whole_screen()),
    ];

    assert_eq!(
        front_foreign_window(&stack, &point(), TARGET, not_a_tool),
        None
    );
}

#[test]
fn no_target_window_and_no_foreign_window_gives_no_evidence() {
    assert_eq!(
        front_foreign_window(&[], &point(), TARGET, not_a_tool),
        None
    );
}

fn bounds(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> CFDictionary<CFString, core_foundation::base::CFType> {
    CFDictionary::from_CFType_pairs(&[
        (CFString::new("X"), CFNumber::from(x).as_CFType()),
        (CFString::new("Y"), CFNumber::from(y).as_CFType()),
        (CFString::new("Width"), CFNumber::from(width).as_CFType()),
        (CFString::new("Height"), CFNumber::from(height).as_CFType()),
    ])
}

fn window(pid: i32, layer: i32, alpha: f64) -> WindowDictionary {
    CFDictionary::from_CFType_pairs(&[
        (
            CFString::new("kCGWindowOwnerPID"),
            CFNumber::from(pid).as_CFType(),
        ),
        (
            CFString::new("kCGWindowLayer"),
            CFNumber::from(layer).as_CFType(),
        ),
        (
            CFString::new("kCGWindowAlpha"),
            CFNumber::from(alpha).as_CFType(),
        ),
        (
            CFString::new("kCGWindowBounds"),
            bounds(10, 20, 300, 200).as_CFType(),
        ),
    ])
}

#[test]
fn the_stack_keeps_order_and_drops_windows_with_no_alpha() {
    let dictionaries = vec![
        window(OTHER, 3, 0.0),
        window(OTHER, 3, 0.5),
        window(TARGET, 0, 1.0),
    ];

    let stack = stack_from(&dictionaries);

    assert_eq!(
        stack,
        vec![
            (OTHER, 3, rect(10.0, 20.0, 300.0, 200.0)),
            (TARGET, 0, rect(10.0, 20.0, 300.0, 200.0)),
        ]
    );
}

#[test]
fn a_window_with_a_missing_field_is_dropped() {
    let partial: WindowDictionary = CFDictionary::from_CFType_pairs(&[(
        CFString::new("kCGWindowAlpha"),
        CFNumber::from(1.0).as_CFType(),
    )]);

    assert!(stack_from(&[partial]).is_empty());
}
