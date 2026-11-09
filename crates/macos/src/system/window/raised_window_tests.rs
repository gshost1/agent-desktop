use super::raised_numbers_from;
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
fn only_raised_windows_of_the_target_process_are_read() {
    let windows = vec![
        window(41, 0, 1),
        window(41, 101, 2),
        window(41, 25, 3),
        window(52, 101, 4),
    ];

    assert_eq!(raised_numbers_from(&windows, 41), vec![2, 3]);
    assert!(raised_numbers_from(&windows, 99).is_empty());
}

#[test]
fn a_window_without_a_layer_is_not_raised() {
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

    assert!(raised_numbers_from(&[bare], 41).is_empty());
}
