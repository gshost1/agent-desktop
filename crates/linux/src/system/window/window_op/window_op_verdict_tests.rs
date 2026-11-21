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

#[test]
fn requested_move_omits_unrequested_size() {
    let requested = requested_of(&move_to(-40, 0));
    assert!(requested.get("width").is_none());
    assert!(requested.get("height").is_none());
    assert_eq!(requested, json!({ "x": -40, "y": 0 }));
}

#[test]
fn requested_resize_omits_unrequested_position() {
    let requested = requested_of(&WindowWrite::MoveResize {
        x: None,
        y: None,
        width: Some(804),
        height: Some(599),
    });
    assert!(requested.get("x").is_none());
    assert!(requested.get("y").is_none());
    assert_eq!(requested, json!({ "width": 804, "height": 599 }));
}

#[test]
fn requested_empty_rectangle_omits_every_field() {
    assert_eq!(
        requested_of(&WindowWrite::MoveResize {
            x: None,
            y: None,
            width: None,
            height: None,
        }),
        json!({})
    );
}

#[test]
fn unverified_request_keeps_observed_rectangle_and_display_bounds() {
    let observed = facts(10, 20, 640, 480);
    let display = Rect {
        x: 0.0,
        y: 0.0,
        width: 1280.0,
        height: 800.0,
    };
    let error = unverified(&move_to(-40, 0), Some(&observed), &[display]);
    let details = error.details.unwrap();
    assert_eq!(details["requested"], json!({ "x": -40, "y": 0 }));
    assert_eq!(
        details["observed"],
        json!({
            "x": 10, "y": 20, "width": 640, "height": 480,
            "hidden": false, "maximized_vertical": false, "maximized_horizontal": false,
        })
    );
    assert_eq!(
        details["display_bounds"],
        json!([{ "x": 0.0, "y": 0.0, "width": 1280.0, "height": 800.0 }])
    );
}
