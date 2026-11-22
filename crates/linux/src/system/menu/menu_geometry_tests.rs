use super::*;
use crate::system::menu::menu_fixture::{BUTTON, ITEM, MENU, PANEL, POPUP_MENU, frame_with, node};
use crate::tree::Field;

#[test]
fn structural_reads_only_need_geometry_for_visible_only_menu_items() {
    let mut visible = node(ITEM, "Open", &[VISIBLE], vec![]);
    visible.payload.node.extents = Field::Failed("field not requested".into());
    let mut hidden = visible.clone();
    hidden.payload.node.traits.states = Field::Present(vec![0, 0]);
    let mut showing = visible.clone();
    showing.payload.node.traits.states = Field::Present(vec![1 << SHOWING, 0]);
    let mut ordinary = visible.clone();
    ordinary.payload.node.traits.role = Field::Present(BUTTON);
    let mut known = visible.clone();
    known.payload.node.extents = Field::Absent;
    let mut web = visible.clone();
    web.in_web_document = true;
    let mut frame = frame_with(vec![node(
        MENU,
        "File",
        &[SHOWING],
        vec![visible, hidden, showing, ordinary, known, web],
    )]);
    let mut items = Vec::new();
    collect(&mut frame, false, &mut items);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].node.properties.name.present().unwrap(), "Open");
}

#[test]
fn closed_or_unread_nearest_menus_never_request_item_bounds() {
    for role in [MENU, POPUP_MENU] {
        for states in [
            Field::Present(vec![0, 0]),
            Field::Absent,
            Field::Failed("unread".into()),
        ] {
            let mut item = node(ITEM, "Closed item", &[VISIBLE], vec![]);
            item.payload.node.extents = Field::Failed("field not requested".into());
            let mut closed = node(role, "Closed", &[], vec![node(PANEL, "", &[], vec![item])]);
            closed.payload.node.traits.states = states;
            let mut tree = frame_with(vec![node(MENU, "Open", &[SHOWING], vec![closed])]);
            let mut items = Vec::new();
            collect(&mut tree, false, &mut items);
            assert!(items.is_empty());
        }
    }
}

#[test]
fn only_the_nearest_menu_controls_geometry_collection() {
    let mut item = node(ITEM, "Nested item", &[VISIBLE], vec![]);
    item.payload.node.extents = Field::Failed("field not requested".into());
    let mut tree = frame_with(vec![
        item.clone(),
        node(
            MENU,
            "Closed",
            &[],
            vec![node(
                POPUP_MENU,
                "Open",
                &[SHOWING],
                vec![node(PANEL, "", &[], vec![item])],
            )],
        ),
    ]);
    let mut items = Vec::new();
    collect(&mut tree, false, &mut items);
    assert_eq!(items.len(), 1);
}
