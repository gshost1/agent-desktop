use super::*;
use crate::system::menu::menu_fixture::*;
use crate::tree::Field;
use crate::tree::vocabulary::state_bits::VISIBLE;
use std::cell::RefCell;

fn rect(x: i32, y: i32, width: i32, height: i32) -> Extents {
    Extents {
        x,
        y,
        width,
        height,
    }
}

fn visible_item(bounds: Field<Extents>) -> Node {
    let mut item = node(ITEM, "Open", &[VISIBLE], vec![]);
    item.payload.node.extents = bounds;
    item
}

#[test]
fn a_visible_only_menu_requires_a_popup_covering_real_item_bounds() {
    let item = visible_item(Field::Present(rect(110, 120, 80, 20)));
    let menu = node(MENU, "File", &[SHOWING], vec![item]);
    let frame = frame_with(vec![node(MENU_BAR, "", &[SHOWING], vec![menu])]);
    let mut presented = corroborate_with(|pid| {
        assert_eq!(pid, 1);
        Some(vec![PopupWindow {
            id: 8,
            bounds: rect(100, 100, 100, 100),
        }])
    });
    let found = super::super::menu_popup::in_window_with(&frame, &mut presented);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].items, 1);
    for popups in [
        None,
        Some(vec![]),
        Some(vec![PopupWindow {
            id: 8,
            bounds: rect(300, 300, 100, 100),
        }]),
    ] {
        let mut closed = corroborate_with(|_| popups.clone());
        assert!(super::super::menu_popup::in_window_with(&frame, &mut closed).is_empty());
    }
}

#[test]
fn sentinel_empty_unread_and_hidden_items_never_use_native_evidence() {
    let mut presented = corroborate_with(|_| panic!("invalid geometry must not read X"));
    for bounds in [
        Field::Present(rect(i32::MIN, 20, 1, 1)),
        Field::Present(rect(10, i32::MIN, 1, 1)),
        Field::Present(rect(10, 20, 0, 20)),
        Field::Present(rect(10, 20, 10, 0)),
        Field::Present(rect(10, 20, -1, 20)),
        Field::Absent,
        Field::Failed("GetExtents failed".into()),
    ] {
        assert!(!presented(&visible_item(bounds)));
    }
    let mut hidden = visible_item(Field::Present(rect(10, 20, 10, 20)));
    hidden.payload.node.traits.states = Field::Present(vec![0, 0]);
    assert!(!presented(&hidden));
}

#[test]
fn popup_evidence_is_cached_per_process_and_requires_full_coverage() {
    let calls = RefCell::new(Vec::new());
    let mut presented = corroborate_with(|pid| {
        calls.borrow_mut().push(pid);
        (pid == 1).then_some(vec![PopupWindow {
            id: 8,
            bounds: rect(-20, -20, 100, 100),
        }])
    });
    assert!(presented(&visible_item(Field::Present(rect(
        -10, -10, 90, 20
    )))));
    for bounds in [
        rect(-21, 0, 10, 10),
        rect(0, -21, 10, 10),
        rect(70, 0, 11, 10),
        rect(0, 70, 10, 11),
        rect(i32::MAX, 0, 20, 10),
    ] {
        assert!(!presented(&visible_item(Field::Present(bounds))));
    }
    let mut foreign = visible_item(Field::Present(rect(0, 0, 20, 20)));
    foreign.payload.node.element = crate::tree::AtspiElement::new(":1.2", "/foreign", 2);
    assert!(!presented(&foreign));
    assert!(!presented(&foreign));
    assert_eq!(*calls.borrow(), [1, 2]);
}

#[test]
fn showing_items_do_not_require_x11_corroboration() {
    let frame = frame_with(vec![gtk3_bar(true)]);
    let found =
        super::super::menu_popup::in_window_with(&frame, &mut |_| panic!("SHOWING is sufficient"));
    assert_eq!(found[0].items, 2);
}

#[test]
fn closed_nearest_menus_never_read_native_popup_evidence() {
    let item = visible_item(Field::Present(rect(110, 120, 80, 20)));
    let closed = node(MENU, "Closed", &[], vec![item.clone()]);
    let tree = frame_with(vec![
        closed.clone(),
        node(MENU, "Open", &[SHOWING], vec![closed]),
    ]);
    let mut presented = corroborate_with(|_| panic!("a closed menu must not read X"));
    assert!(super::super::menu_popup::in_window_with(&tree, &mut presented).is_empty());
}

#[test]
fn top_level_children_share_one_native_read_per_process() {
    let calls = std::rc::Rc::new(RefCell::new(Vec::new()));
    let rejected = node(
        MENU,
        "Uncovered",
        &[SHOWING],
        vec![visible_item(Field::Present(rect(400, 400, 80, 20)))],
    );
    let accepted = node(
        MENU,
        "Covered",
        &[SHOWING],
        vec![visible_item(Field::Present(rect(110, 120, 80, 20)))],
    );
    let window = node(WINDOW, "", &[SHOWING], vec![rejected, accepted]);
    let found = super::super::menu_popup::top_level_with(&window, || {
        let calls = std::rc::Rc::clone(&calls);
        corroborate_with(move |pid| {
            calls.borrow_mut().push(pid);
            Some(vec![PopupWindow {
                id: 8,
                bounds: rect(100, 100, 100, 100),
            }])
        })
    })
    .unwrap();
    assert_eq!(
        found.root.payload.node.properties.name.present().unwrap(),
        "Covered"
    );
    assert_eq!(*calls.borrow(), [1]);
}
