use super::*;
use crate::system::menu::menu_fixture::{BUTTON, ITEM, frame_with, node};
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
    let mut frame = frame_with(vec![visible, hidden, showing, ordinary, known, web]);
    let mut items = Vec::new();
    collect(&mut frame, &mut items);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].node.properties.name.present().unwrap(), "Open");
}
