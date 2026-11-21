use super::*;
use crate::tree::Field;
use crate::tree::live::test_support::{read_of, words};
use crate::tree::vocabulary::role_numbers::{DOCUMENT_FRAME, LABEL, PARAGRAPH};

fn node(role: u32, path: &str, states: &[u32]) -> WalkedNode<ObservedRead> {
    let mut read = read_of(role, "", &[]);
    read.node.element = AtspiElement::new(":1.1", path, 1);
    read.node.traits.states = Field::Present(words(states));
    read.node.traits.interfaces = Field::Present(vec![TEXT_INTERFACE.into(), EDITABLE_TEXT.into()]);
    WalkedNode {
        payload: read,
        children: vec![],
        truncated: false,
        child_count: None,
        source_index: 0,
        in_web_document: false,
    }
}

fn document(children: Vec<WalkedNode<ObservedRead>>) -> Walk<ObservedRead> {
    let mut root = node(DOCUMENT_FRAME, "/document", &[EDITABLE]);
    root.payload.node.traits.interfaces = Field::Present(vec![]);
    root.children = children;
    Walk {
        root,
        complete: true,
        node_count: 2,
        max_depth: 1,
        counts: Default::default(),
    }
}

#[test]
fn document_input_uses_only_the_focused_writable_text_descendant() {
    let walk = document(vec![
        node(PARAGRAPH, "/other", &[EDITABLE]),
        node(PARAGRAPH, "/caret", &[EDITABLE, FOCUSED]),
    ]);
    assert_eq!(focused_editor(&walk).unwrap().path(), "/caret");
}

#[test]
fn document_input_refuses_empty_ambiguous_read_only_or_incomplete_evidence() {
    let focused = || node(PARAGRAPH, "/caret", &[EDITABLE, FOCUSED]);
    assert!(focused_editor(&document(vec![])).is_err());
    assert!(focused_editor(&document(vec![node(PARAGRAPH, "/p", &[EDITABLE])])).is_err());
    assert!(
        focused_editor(&document(vec![
            focused(),
            node(PARAGRAPH, "/second", &[EDITABLE, FOCUSED])
        ]))
        .is_err()
    );
    assert!(focused_editor(&document(vec![node(PARAGRAPH, "/p", &[FOCUSED])])).is_err());
    assert!(
        focused_editor(&document(vec![node(
            PARAGRAPH,
            "/p",
            &[EDITABLE, FOCUSED, READ_ONLY]
        )]))
        .is_err()
    );
    assert!(focused_editor(&document(vec![node(LABEL, "/label", &[EDITABLE, FOCUSED])])).is_err());
    let mut walk = document(vec![focused()]);
    walk.complete = false;
    assert!(focused_editor(&walk).is_err());
    walk.complete = true;
    walk.root.payload.node.traits.states = Field::Present(words(&[EDITABLE, READ_ONLY]));
    assert!(focused_editor(&walk).is_err());
}

#[test]
fn document_input_refuses_failed_states_or_missing_interfaces() {
    let mut child = node(PARAGRAPH, "/p", &[EDITABLE, FOCUSED]);
    child.payload.node.traits.interfaces = Field::Present(vec![TEXT_INTERFACE.into()]);
    assert!(focused_editor(&document(vec![child.clone()])).is_err());
    child.payload.node.traits.states = Field::Failed("NoReply".into());
    assert!(focused_editor(&document(vec![child])).is_err());
    let mut unknown = node(PARAGRAPH, "/unknown", &[EDITABLE, FOCUSED]);
    unknown.payload.node.traits.interfaces = Field::Failed("NoReply".into());
    assert!(
        focused_editor(&document(vec![
            node(PARAGRAPH, "/known", &[EDITABLE, FOCUSED]),
            unknown,
        ]))
        .is_err()
    );
}
