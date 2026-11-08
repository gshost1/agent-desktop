use super::*;
use crate::tree::Extents;

fn facts(x: i32, y: i32, width: i32, height: i32) -> WindowFacts {
    WindowFacts {
        client: Extents {
            x,
            y,
            width,
            height,
        },
        hidden: false,
        maximized_vertical: false,
        maximized_horizontal: false,
    }
}

fn move_to(x: i32, y: i32) -> WindowWrite {
    WindowWrite::MoveResize {
        x: Some(x),
        y: Some(y),
        width: None,
        height: None,
    }
}

#[test]
fn readback_tolerance_is_two_pixels_at_the_boundary() {
    let requested = WindowWrite::MoveResize {
        x: None,
        y: None,
        width: Some(562),
        height: Some(693),
    };
    assert!(holds(&requested, &facts(10, 10, 564, 693)));
    assert!(!holds(&requested, &facts(10, 10, 565, 693)));
}

#[test]
fn a_negative_coordinate_is_compared_as_a_signed_value() {
    assert!(holds(&move_to(-40, -10), &facts(-40, -10, 5, 5)));
    assert!(!holds(&move_to(-40, -10), &facts(40, 10, 5, 5)));
}

#[test]
fn maximize_needs_both_atoms() {
    let mut half = facts(0, 0, 1, 1);
    half.maximized_vertical = true;
    assert!(!holds(&WindowWrite::SetMaximized(true), &half));
    half.maximized_horizontal = true;
    assert!(holds(&WindowWrite::SetMaximized(true), &half));
}

#[test]
fn restore_from_maximized_needs_neither_maximized_atom() {
    let mut window = facts(0, 0, 1, 1);
    window.maximized_horizontal = true;
    assert!(!holds(&WindowWrite::SetMaximized(false), &window));
    window.maximized_horizontal = false;
    assert!(holds(&WindowWrite::SetMaximized(false), &window));
}

#[test]
fn a_resize_is_judged_by_its_size_alone() {
    let resize = WindowWrite::MoveResize {
        x: None,
        y: None,
        width: Some(804),
        height: Some(599),
    };
    assert!(holds(&resize, &facts(440, 201, 804, 599)));
    assert!(!holds(&resize, &facts(0, 0, 804, 650)));
}
