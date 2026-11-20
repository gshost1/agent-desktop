use agent_desktop_core::LocatorField;

use super::HeadlessRoutes;
use crate::system::process::toolkit_id::ToolkitId;
use crate::tree::element::ancestry;
use crate::tree::live::test_support::{read_of, words};
use crate::tree::observe::projection::project_node;
use crate::tree::vocabulary::role_numbers::{COMBO_BOX, LIST_ITEM};
use crate::tree::vocabulary::state_bits::{SELECTABLE, SENSITIVE};
use crate::tree::{AtspiElement, Field, ObservedRead, WalkLimits, WalkedNode};

fn toolkit(name: &str) -> ToolkitId {
    ToolkitId {
        name: Some(name.into()),
        version: Some("6.4.2".into()),
    }
}

fn node(role: u32, in_combo: bool) -> WalkedNode<ObservedRead> {
    let mut read = read_of(role, "choice", &["select"]);
    read.node.traits.states = Field::Present(words(&[SENSITIVE, SELECTABLE]));
    if in_combo {
        let owner = AtspiElement::new(":1.1", "/combo", 1);
        read.node
            .element
            .set_ancestry(ancestry::child(&owner, Some(COMBO_BOX)));
    }
    WalkedNode {
        payload: read,
        children: Vec::new(),
        truncated: false,
        child_count: None,
        source_index: 0,
        in_web_document: false,
    }
}

fn offers_select(node: &WalkedNode<ObservedRead>, routes: &HeadlessRoutes) -> bool {
    let mut evidence = project_node(&node.payload, false, &WalkLimits::default());
    routes.withhold(node, &mut evidence);
    match evidence.ref_evidence.available_actions {
        LocatorField::Known(tokens) => tokens.iter().any(|token| token == "Select"),
        other => panic!("tokens unread: {other:?}"),
    }
}

#[test]
fn qt_combo_and_its_choices_do_not_promise_a_headless_select_even_with_an_own_verb() {
    let routes = HeadlessRoutes::of_application(":1.1", toolkit("Qt"));
    for target in [node(COMBO_BOX, false), node(LIST_ITEM, true)] {
        assert!(!offers_select(&target, &routes));
        assert!(offers_select(&target, &HeadlessRoutes::unread()));
    }
}

#[test]
fn qt_rows_outside_a_combo_keep_their_semantic_route() {
    assert!(offers_select(
        &node(LIST_ITEM, false),
        &HeadlessRoutes::of_application(":1.1", toolkit("Qt"))
    ));
}

#[test]
fn other_toolkits_and_other_applications_do_not_inherit_qt_combo_restrictions() {
    let target = node(LIST_ITEM, true);
    for name in ["GTK", "Chromium", "Gecko"] {
        assert!(offers_select(
            &target,
            &HeadlessRoutes::of_application(":1.1", toolkit(name))
        ));
    }
    assert!(offers_select(
        &target,
        &HeadlessRoutes::of_application(":1.9", toolkit("Qt"))
    ));
}
