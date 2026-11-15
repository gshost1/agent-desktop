use agent_desktop_core::LocatorField;

use super::{HeadlessRoutes, withheld_tokens};
use crate::system::process::toolkit_id::ToolkitId;
use crate::tree::live::test_support::{read_of, words};
use crate::tree::observe::projection::project_node;
use crate::tree::vocabulary::role_numbers::{
    COMBO_BOX, ENTRY, LIST, LIST_ITEM, PAGE_TAB, PUSH_BUTTON,
};
use crate::tree::vocabulary::state_bits::{
    CHECKABLE, EDITABLE, FOCUSABLE, HAS_POPUP, SELECTABLE, SENSITIVE,
};
use crate::tree::{Field, ObservedRead, WalkLimits, WalkedNode};

const PAGE_TAB_LIST: u32 = 38;
const SELECTION: &str = "org.a11y.atspi.Selection";
const EDITABLE_TEXT: &str = "org.a11y.atspi.EditableText";
const TEXT_INTERFACE: &str = "org.a11y.atspi.Text";

fn toolkit(name: &str, version: &str) -> ToolkitId {
    ToolkitId {
        name: Some(name.into()),
        version: Some(version.into()),
    }
}

fn gtk4() -> ToolkitId {
    toolkit("GTK", "4.14.5")
}

fn read(
    role: u32,
    states: &[u32],
    interfaces: &[&str],
    names: &[&str],
    children: i32,
) -> ObservedRead {
    let mut read = read_of(role, "n", names);
    read.node.traits.states = Field::Present(words(states));
    read.node.traits.interfaces =
        Field::Present(interfaces.iter().map(|name| (*name).to_owned()).collect());
    read.node.properties.child_count = Field::Present(children);
    read
}

fn entry() -> ObservedRead {
    read(
        ENTRY,
        &[SENSITIVE, EDITABLE, FOCUSABLE, HAS_POPUP],
        &[EDITABLE_TEXT, TEXT_INTERFACE],
        &["activate", "activate-primary-icon"],
        0,
    )
}

fn tab() -> ObservedRead {
    read(PAGE_TAB, &[SENSITIVE, SELECTABLE], &[], &[], 1)
}

fn tab_list() -> ObservedRead {
    read(PAGE_TAB_LIST, &[SENSITIVE], &[SELECTION], &[], 2)
}

fn row() -> ObservedRead {
    read(LIST_ITEM, &[SENSITIVE, SELECTABLE], &[], &[], 1)
}

fn list() -> ObservedRead {
    read(LIST, &[SENSITIVE], &[SELECTION], &[], 3)
}

fn leaf(payload: ObservedRead) -> WalkedNode<ObservedRead> {
    holding(payload, Vec::new())
}

fn holding(
    payload: ObservedRead,
    children: Vec<WalkedNode<ObservedRead>>,
) -> WalkedNode<ObservedRead> {
    WalkedNode {
        payload,
        children,
        truncated: false,
        child_count: None,
        source_index: 0,
        in_web_document: false,
    }
}

fn withheld(read: ObservedRead, toolkit: &ToolkitId) -> Vec<&'static str> {
    withheld_tokens(&leaf(read), toolkit)
}

fn tokens(read: &ObservedRead, routes: &HeadlessRoutes) -> Vec<String> {
    let mut evidence = project_node(read, false, &WalkLimits::default());
    routes.withhold(&leaf(read.clone()), &mut evidence);
    match evidence.ref_evidence.available_actions {
        LocatorField::Known(tokens) => tokens,
        other => panic!("tokens unread: {other:?}"),
    }
}

#[test]
fn gtk4_entry_activation_and_index_selects_are_not_promised_to_a_snapshot() {
    let routes = HeadlessRoutes::of_application(":1.1", gtk4());
    assert!(!tokens(&entry(), &routes).contains(&"Click".to_owned()));
    assert!(tokens(&entry(), &routes).contains(&"TypeText".to_owned()));
    for node in [tab(), tab_list(), row(), list()] {
        assert!(
            !tokens(&node, &routes).contains(&"Select".to_owned()),
            "{:?}",
            node.node.traits.role
        );
    }
}

#[test]
fn live_evidence_keeps_the_tokens_a_headed_route_needs() {
    let unread = HeadlessRoutes::unread();
    assert!(tokens(&entry(), &unread).contains(&"Click".to_owned()));
    for node in [tab(), tab_list(), row(), list()] {
        assert!(tokens(&node, &unread).contains(&"Select".to_owned()));
    }
}

#[test]
fn a_node_of_another_application_keeps_its_tokens() {
    let routes = HeadlessRoutes::of_application(":1.9", gtk4());
    assert!(tokens(&tab(), &routes).contains(&"Select".to_owned()));
    assert!(tokens(&entry(), &routes).contains(&"Click".to_owned()));
}

#[test]
fn toolkits_with_index_safe_selection_keep_select() {
    for toolkit in [
        toolkit("GTK", "3.24.41"),
        toolkit("Qt", "6.4.2"),
        toolkit("Chromium", "126"),
        toolkit("Gecko", "128"),
    ] {
        for node in [tab(), tab_list(), row(), list()] {
            assert!(withheld(node, &toolkit).is_empty(), "{toolkit:?}");
        }
        assert!(withheld(entry(), &toolkit).is_empty());
    }
}

#[test]
fn a_toolkit_that_names_nothing_refuses_index_selects_as_the_action_does() {
    let unnamed = ToolkitId::default();
    assert_eq!(withheld(tab(), &unnamed), vec!["Select"]);
    assert!(withheld(entry(), &unnamed).is_empty());
}

#[test]
fn gtk4_routes_that_land_headless_stay_promised() {
    let gtk = gtk4();
    let own_verb = read(LIST_ITEM, &[SENSITIVE, SELECTABLE], &[], &["select"], 1);
    assert!(withheld(own_verb, &gtk).is_empty());
    let toggled_row = read(LIST_ITEM, &[SENSITIVE, SELECTABLE], &[], &["toggle"], 1);
    assert!(withheld(toggled_row, &gtk).is_empty());
    let checkable_row = read(
        LIST_ITEM,
        &[SENSITIVE, SELECTABLE, CHECKABLE],
        &[],
        &["toggle"],
        1,
    );
    assert_eq!(withheld(checkable_row, &gtk), vec!["Select"]);
    let dropdown = read(COMBO_BOX, &[SENSITIVE, HAS_POPUP], &[], &[], 1);
    assert!(withheld(dropdown, &gtk).is_empty());
    let pointer_combo = read(COMBO_BOX, &[SENSITIVE], &[], &[], 2);
    assert!(withheld(pointer_combo, &gtk).is_empty());
    let entry_with_click = read(
        ENTRY,
        &[SENSITIVE, EDITABLE],
        &[EDITABLE_TEXT],
        &["activate", "click"],
        0,
    );
    assert!(withheld(entry_with_click, &gtk).is_empty());
}

fn snapshot_actions(routes: &HeadlessRoutes) -> Vec<Vec<String>> {
    use crate::tree::observe::decorate::Plain;
    use crate::tree::observe::projection::project_with;
    use crate::tree::{Walk, WalkCounts, WalkedNode};
    use agent_desktop_core::{ObservationSource, ObservedTree, WindowInfo, WindowState};
    let leaf_at = |payload, source_index| WalkedNode {
        payload,
        children: Vec::new(),
        truncated: false,
        child_count: None,
        source_index,
        in_web_document: false,
    };
    let walk = Walk {
        root: WalkedNode {
            children: vec![leaf_at(entry(), 0), leaf_at(tab(), 1)],
            ..leaf_at(read(LIST, &[SENSITIVE], &[], &[], 2), 0)
        },
        complete: true,
        node_count: 3,
        max_depth: 1,
        counts: WalkCounts::default(),
    };
    let (subtree, stats) = project_with(walk, &WalkLimits::default(), &mut Plain, None, routes);
    let window = WindowInfo {
        id: "w-1.1-p".to_owned(),
        title: "t".to_owned(),
        app: "a".to_owned(),
        pid: 1.into(),
        process_instance: None,
        bounds: None,
        state: WindowState::default(),
    };
    let root = agent_desktop_core::ObservationRoot::Window(&window);
    let tree = ObservedTree::from_roots(
        vec![subtree],
        ObservationSource::from_root(&root, agent_desktop_core::SnapshotSurface::Window),
        stats,
        true,
    )
    .expect("a projected walk builds a tree")
    .into_accessibility_tree_partial()
    .expect("one root projects")
    .0;
    tree.children
        .iter()
        .map(|child| child.presentation.available_actions.clone())
        .collect()
}

#[test]
fn a_gtk4_snapshot_prints_only_the_tokens_a_headless_request_can_deliver() {
    let snapshot = snapshot_actions(&HeadlessRoutes::of_application(":1.1", gtk4()));
    assert!(!snapshot[0].contains(&"Click".to_owned()), "{snapshot:?}");
    assert!(snapshot[0].contains(&"SetValue".to_owned()), "{snapshot:?}");
    assert!(!snapshot[1].contains(&"Select".to_owned()), "{snapshot:?}");
    let unread = snapshot_actions(&HeadlessRoutes::unread());
    assert!(unread[0].contains(&"Click".to_owned()), "{unread:?}");
    assert!(unread[1].contains(&"Select".to_owned()), "{unread:?}");
}

fn label(names: &[&str]) -> WalkedNode<ObservedRead> {
    leaf(read(PUSH_BUTTON, &[SENSITIVE], &[], names, 0))
}

#[test]
fn a_gtk4_container_keeps_select_when_an_item_it_can_match_acts_on_its_own() {
    let gtk = gtk4();
    let row_of = |names: &[&str]| {
        holding(
            read(LIST_ITEM, &[SENSITIVE, SELECTABLE], &[], names, 1),
            vec![label(&[])],
        )
    };
    let plain = holding(list(), vec![row_of(&[]), row_of(&[])]);
    assert_eq!(withheld_tokens(&plain, &gtk), vec!["Select"]);
    let activatable = holding(list(), vec![row_of(&[]), row_of(&["activate"])]);
    assert!(withheld_tokens(&activatable, &gtk).is_empty());
    let nested_select = holding(
        list(),
        vec![holding(
            read(LIST_ITEM, &[SENSITIVE, SELECTABLE], &[], &[], 1),
            vec![label(&["select"])],
        )],
    );
    assert!(withheld_tokens(&nested_select, &gtk).is_empty());
    let nested_click = holding(
        list(),
        vec![holding(
            read(LIST_ITEM, &[SENSITIVE, SELECTABLE], &[], &[], 1),
            vec![label(&["click"])],
        )],
    );
    assert_eq!(withheld_tokens(&nested_click, &gtk), vec!["Select"]);
    let mut unread = read(LIST_ITEM, &[SENSITIVE, SELECTABLE], &[], &[], 1);
    unread.node.actions = Field::Failed("timed out".into());
    let unread_row = holding(list(), vec![leaf(unread)]);
    assert!(withheld_tokens(&unread_row, &gtk).is_empty());
    let clickable_label_in_row = holding(
        read(LIST_ITEM, &[SENSITIVE, SELECTABLE], &[], &[], 1),
        vec![label(&["click"])],
    );
    assert_eq!(
        withheld_tokens(&clickable_label_in_row, &gtk),
        vec!["Select"]
    );
}

fn combo_choice() -> ObservedRead {
    use crate::tree::AtspiElement;
    use crate::tree::element::ancestry;
    let combo = AtspiElement::new(":1.1", "/combo", 1);
    let mut choice = read(LIST_ITEM, &[SENSITIVE, SELECTABLE], &[], &[], 0);
    choice.node.element = AtspiElement::new(":1.1", "/combo/alpha", 1);
    choice
        .node
        .element
        .set_ancestry(ancestry::child(&combo, Some(COMBO_BOX)));
    choice
}

#[test]
fn a_qt_combo_choice_is_not_promised_a_headless_select() {
    let qt = toolkit("Qt", "6.6.1");
    assert!(tokens(&combo_choice(), &HeadlessRoutes::unread()).contains(&"Select".to_owned()));
    let routes = HeadlessRoutes::of_application(":1.1", qt.clone());
    assert!(!tokens(&combo_choice(), &routes).contains(&"Select".to_owned()));
    assert_eq!(withheld(combo_choice(), &qt), vec!["Select"]);
    assert!(withheld(row(), &qt).is_empty());
    assert!(withheld(combo_choice(), &toolkit("GTK", "3.24.41")).is_empty());
}
