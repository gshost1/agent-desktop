//! `read` driven by scripted batch replies: which replies hand the element to
//! the one-at-a-time read, which fail the search, and how often the batch is
//! asked.

use super::*;
use core_foundation::number::CFNumber;
use core_foundation_sys::base::CFTypeRef;
use std::cell::Cell;
use std::time::Duration;

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(5)
}

fn owned(values: &[CFType]) -> CFTypeRef {
    let array = CFArray::from_CFTypes(values);
    unsafe { core_foundation::base::CFRetain(array.as_CFTypeRef()) }
}

fn reply(error: i32, values: &[CFType]) -> (i32, CFTypeRef) {
    (error, owned(values))
}

fn role(name: &str) -> CFType {
    CFString::new(name).as_CFType()
}

fn empty() -> CFType {
    CFArray::<CFType>::from_CFTypes(&[]).as_CFType()
}

fn answer(
    replies: (i32, CFTypeRef),
    visible: Option<bool>,
) -> (Result<Option<MenuSearchNode>, AdapterError>, usize, bool) {
    let asked = Cell::new(0);
    let visibility_read = Cell::new(false);
    let mut pending = Some(replies);
    let result = read_with(
        || {
            asked.set(asked.get() + 1);
            pending
                .take()
                .unwrap_or((accessibility_sys::kAXErrorCannotComplete, std::ptr::null()))
        },
        || {
            visibility_read.set(true);
            Ok(visible)
        },
        deadline(),
    );
    (result, asked.get(), visibility_read.get())
}

#[test]
fn a_busy_element_is_asked_once_and_left_to_the_attribute_read() {
    for error in [
        accessibility_sys::kAXErrorCannotComplete,
        accessibility_sys::kAXErrorFailure,
        accessibility_sys::kAXErrorInvalidUIElement,
    ] {
        let (result, asked, _) = answer((error, std::ptr::null()), None);
        assert!(result.expect("no search error").is_none());
        assert_eq!(asked, 1);
    }
}

#[test]
fn an_error_reply_that_carries_a_value_is_still_left_to_the_attribute_read() {
    let (result, asked, _) = answer(
        reply(
            accessibility_sys::kAXErrorFailure,
            &[role("AXGroup"), empty()],
        ),
        None,
    );
    assert!(result.expect("no search error").is_none());
    assert_eq!(asked, 1);
}

#[test]
fn a_disabled_accessibility_api_fails_the_search() {
    let (result, asked, visibility_read) = answer(
        (accessibility_sys::kAXErrorAPIDisabled, std::ptr::null()),
        None,
    );
    let error = result.err().expect("a search error");
    assert_eq!(error.code, agent_desktop_core::ErrorCode::PermDenied);
    assert_eq!(asked, 1);
    assert!(!visibility_read);
}

#[test]
fn a_reply_of_the_wrong_shape_is_left_to_the_attribute_read() {
    let success = accessibility_sys::kAXErrorSuccess;
    for values in [
        vec![role("AXGroup")],
        vec![role("AXGroup"), empty(), empty()],
        vec![CFNumber::from(1_i32).as_CFType(), empty()],
    ] {
        let (result, _, _) = answer(reply(success, &values), None);
        assert!(result.expect("no search error").is_none());
    }
    let (result, _, _) = answer((success, std::ptr::null()), None);
    assert!(result.expect("no search error").is_none());
    let number = CFNumber::from(1_i32);
    let not_an_array = unsafe { core_foundation::base::CFRetain(number.as_CFTypeRef()) };
    let (result, _, _) = answer((success, not_an_array), None);
    assert!(result.expect("no search error").is_none());
}

#[test]
fn only_a_menu_asks_for_visibility_and_a_hidden_menu_hands_back_its_children() {
    let success = accessibility_sys::kAXErrorSuccess;
    let (result, _, visibility_read) = answer(reply(success, &[role("AXGroup"), empty()]), None);
    assert!(!result.expect("no error").expect("a node").is_menu);
    assert!(!visibility_read);
    let (result, _, visibility_read) =
        answer(reply(success, &[role("AXMenu"), empty()]), Some(true));
    assert!(result.expect("no error").expect("a node").is_menu);
    assert!(visibility_read);
    let (result, _, _) = answer(reply(success, &[role("AXMenu"), empty()]), Some(false));
    assert!(!result.expect("no error").expect("a node").is_menu);
}

#[test]
fn a_passed_deadline_asks_nothing() {
    let asked = Cell::new(0);
    let result = read_with(
        || {
            asked.set(asked.get() + 1);
            (accessibility_sys::kAXErrorSuccess, std::ptr::null())
        },
        || Ok(None),
        Instant::now(),
    );
    assert_eq!(
        result.err().expect("a timeout").code,
        agent_desktop_core::ErrorCode::Timeout
    );
    assert_eq!(asked.get(), 0);
}
