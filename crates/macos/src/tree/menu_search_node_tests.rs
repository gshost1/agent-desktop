use super::*;
use core_foundation::number::CFNumber;

fn role(name: &str) -> Option<CFType> {
    Some(CFString::new(name).as_CFType())
}

fn visible(value: bool) -> Option<CFType> {
    Some(CFBoolean::from(value).as_CFType())
}

fn no_children() -> Option<CFType> {
    let empty: CFArray<CFType> = CFArray::from_CFTypes(&[]);
    Some(empty.as_CFType())
}

#[test]
fn a_menu_that_does_not_say_it_is_hidden_is_the_answer() {
    let shown = decode(&[role("AXMenu"), None, no_children()]).expect("a node");
    assert!(shown.is_menu);
    let visible_menu = decode(&[role("AXMenu"), visible(true), no_children()]).expect("a node");
    assert!(visible_menu.is_menu);
}

#[test]
fn a_hidden_menu_a_menu_bar_and_other_roles_are_not_menus() {
    for slots in [
        [role("AXMenu"), visible(false), no_children()],
        [role("AXMenuBar"), visible(true), no_children()],
        [role("AXGroup"), visible(true), no_children()],
        [None, None, None],
    ] {
        assert!(!decode(&slots).expect("a node").is_menu);
    }
}

#[test]
fn a_children_slot_that_is_not_a_list_reads_as_no_children() {
    let node = decode(&[
        role("AXGroup"),
        None,
        Some(CFNumber::from(3_i32).as_CFType()),
    ])
    .expect("a node");
    assert!(node.children.is_empty());
}

fn numbers(count: usize) -> Option<CFType> {
    let items: Vec<CFType> = (0..count)
        .map(|index| CFNumber::from(index as i32).as_CFType())
        .collect();
    Some(CFArray::from_CFTypes(&items).as_CFType())
}

#[test]
fn a_child_list_past_one_observation_fails_as_incomplete_not_as_empty() {
    let limit = agent_desktop_core::ObservationBudget::default().max_children_per_node;
    let Err(error) = decode(&[role("AXGroup"), None, numbers(limit + 1)]) else {
        panic!("an oversized child list must fail");
    };
    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    let details = error.details.expect("details");
    assert_eq!(details["kind"], "surface_array_incomplete");
    assert_eq!(details["total_count"], limit + 1);
}

#[test]
fn a_child_list_holding_something_other_than_elements_fails_rather_than_shrinking() {
    assert!(decode(&[role("AXGroup"), None, numbers(2)]).is_err());
}

#[test]
fn the_batch_asks_for_role_visibility_and_children_in_slot_order() {
    let names = attribute_names();
    let read: Vec<String> = names.iter().map(|name| name.to_string()).collect();
    assert_eq!(read, ["AXRole", "AXVisible", "AXChildren"]);
    assert_eq!((ROLE, VISIBLE, CHILDREN), (0, 1, 2));
}

#[test]
fn an_element_that_answers_the_batch_with_absence_is_an_empty_node() {
    for error in [
        accessibility_sys::kAXErrorFailure,
        accessibility_sys::kAXErrorAttributeUnsupported,
        accessibility_sys::kAXErrorNoValue,
    ] {
        let node = answer(error, None).expect("an empty node");
        assert!(!node.is_menu && node.children.is_empty());
    }
}

#[test]
fn a_busy_or_vanished_element_fails_the_search() {
    let busy = answer(accessibility_sys::kAXErrorCannotComplete, None);
    assert_eq!(busy.err().map(|error| error.code), Some(ErrorCode::Timeout));
    let gone = answer(accessibility_sys::kAXErrorInvalidUIElement, None);
    assert_eq!(
        gone.err().map(|error| error.code),
        Some(ErrorCode::AppUnresponsive)
    );
}
