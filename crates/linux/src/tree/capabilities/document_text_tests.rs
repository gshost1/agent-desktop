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
    assert!(focused_editor(&document(vec![node(LABEL, "/label", &[FOCUSED])])).is_err());
    let mut walk = document(vec![focused()]);
    walk.complete = false;
    assert!(focused_editor(&walk).is_err());
    walk.complete = true;
    walk.root.payload.node.traits.states = Field::Present(words(&[EDITABLE, READ_ONLY]));
    assert!(focused_editor(&walk).is_err());
}

#[test]
fn heading_input_uses_capabilities_not_a_descendant_role_allowlist() {
    let walk = document(vec![node(83, "/heading", &[EDITABLE, FOCUSED])]);
    let editor = focused_editor(&walk).unwrap();
    assert_eq!(editor.path(), "/heading");
    assert_eq!(editor.resolved_role(), Some("heading"));
}

#[test]
fn routed_input_details_name_the_observed_descendant() {
    let walk = document(vec![node(83, "/heading", &[EDITABLE, FOCUSED])]);
    let editor = focused_editor(&walk).unwrap();
    let input = InputTarget::new(editor);
    assert_eq!(
        input.details,
        serde_json::json!({"role": "heading", "path": "/heading"})
    );
}

#[test]
fn only_document_handles_need_an_extra_routing_read() {
    assert!(needs_routing(Some("document")));
    assert!(!needs_routing(Some("paragraph")));
    assert!(!needs_routing(Some("textfield")));
    assert!(!needs_routing(None));
}

#[test]
fn spreadsheet_and_presentation_documents_route_to_capable_descendants() {
    for role in [92, 93] {
        let mut walk = document(vec![node(56, "/cell", &[EDITABLE, FOCUSED])]);
        walk.root.payload.node.traits.role = Field::Present(role);
        assert_eq!(focused_editor(&walk).unwrap().path(), "/cell");
    }
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
