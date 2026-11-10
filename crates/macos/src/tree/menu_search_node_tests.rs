use super::*;
use core_foundation::number::CFNumber;

fn role(name: &str) -> CFType {
    CFString::new(name).as_CFType()
}

fn missing() -> CFType {
    unsafe { CFType::wrap_under_get_rule(core_foundation_sys::base::kCFNull as _) }
}

fn error(code: i32) -> CFType {
    let value = unsafe {
        accessibility_sys::AXValueCreate(
            accessibility_sys::kAXValueTypeAXError,
            (&code as *const i32).cast(),
        )
    };
    unsafe { CFType::wrap_under_create_rule(value as _) }
}

fn numbers(count: usize) -> CFType {
    let items: Vec<CFType> = (0..count)
        .map(|index| CFNumber::from(index as i32).as_CFType())
        .collect();
    CFArray::from_CFTypes(&items).as_CFType()
}

fn no_children() -> CFType {
    numbers(0)
}

fn batch(values: [CFType; 2]) -> Option<Batch> {
    decode(&values)
}

#[test]
fn a_menu_bar_is_not_searched_below_and_a_displayed_menu_is_the_answer() {
    let bar = batch([
        role("AXMenuBar"),
        error(accessibility_sys::kAXErrorCannotComplete),
    ])
    .expect("a clean role");
    let bar = search_node(bar, false).expect("a node");
    assert!(!bar.is_menu && bar.children.is_empty());
    let menu = batch([
        role("AXMenu"),
        error(accessibility_sys::kAXErrorCannotComplete),
    ])
    .expect("a clean role");
    assert!(search_node(menu, true).expect("a node").is_menu);
}

#[test]
fn a_hidden_menu_and_other_roles_hand_back_their_children() {
    for role_value in [role("AXMenu"), role("AXGroup"), missing()] {
        let node =
            search_node(batch([role_value, no_children()]).expect("clean"), false).expect("a node");
        assert!(!node.is_menu);
    }
}

#[test]
fn an_unsupported_child_list_is_no_children() {
    let unsupported = error(accessibility_sys::kAXErrorAttributeUnsupported);
    let node =
        search_node(batch([role("AXGroup"), unsupported]).expect("clean"), false).expect("a node");
    assert!(node.children.is_empty());
}

#[test]
fn a_child_list_the_paged_read_must_judge_is_left_to_it() {
    let limit = agent_desktop_core::ObservationBudget::default().max_children_per_node;
    for children in [
        numbers(limit + 1),
        numbers(2),
        CFNumber::from(3_i32).as_CFType(),
        error(accessibility_sys::kAXErrorFailure),
        error(accessibility_sys::kAXErrorCannotComplete),
    ] {
        let read = batch([role("AXGroup"), children]).expect("a clean role");
        assert!(search_node(read, false).is_none());
    }
}

#[test]
fn a_role_the_attribute_read_must_judge_is_left_to_it() {
    for role_value in [
        error(accessibility_sys::kAXErrorCannotComplete),
        error(accessibility_sys::kAXErrorInvalidUIElement),
        CFNumber::from(1_i32).as_CFType(),
    ] {
        assert!(batch([role_value, no_children()]).is_none());
    }
}

#[test]
fn the_batch_asks_for_role_and_children_and_not_visibility() {
    let names = attribute_names();
    let read: Vec<String> = names.iter().map(|name| name.to_string()).collect();
    assert_eq!(read, ["AXRole", "AXChildren"]);
    assert_eq!((ROLE, CHILDREN), (0, 1));
}
