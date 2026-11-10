use super::test_support::{evidence, node, tree, window};
use super::{
    LocatorMaterialization, LocatorResolveRequest, LocatorSelection, ObservationRequest,
    ObservationRoot, ObservedTree, resolve_query,
};
use crate::{
    AdapterError,
    adapter::{ActionOps, InputOps, ObservationOps, SystemOps},
    locator::LocatorQuery,
};
use std::sync::atomic::{AtomicUsize, Ordering};

const ROWS: u32 = 3000;

#[derive(Default)]
struct PanelAdapter {
    passes_without_rows: AtomicUsize,
    passes_with_rows: AtomicUsize,
}

impl ObservationOps for PanelAdapter {
    fn observe_tree(
        &self,
        _root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        if request.observation_mode.leave_rows_unread {
            self.passes_without_rows.fetch_add(1, Ordering::SeqCst);
            return Ok(panel_without_rows());
        }
        self.passes_with_rows.fetch_add(1, Ordering::SeqCst);
        Ok(panel_with_rows())
    }
}

impl ActionOps for PanelAdapter {}
impl InputOps for PanelAdapter {}
impl SystemOps for PanelAdapter {}

fn panel_without_rows() -> ObservedTree {
    let mut list = node(1, evidence("list", None), Vec::new(), &[0]);
    list.completeness.subtree_complete = false;
    list.children_count = Some(ROWS);
    let mut root = node(0, evidence("sheet", None), vec![1, 2], &[]);
    root.completeness.subtree_complete = false;
    let mut panel = tree(
        vec![
            root,
            list,
            node(2, evidence("combobox", Some("Where:")), Vec::new(), &[1]),
        ],
        vec![0],
        false,
    );
    panel.mark_rows_unread();
    panel
}

fn panel_with_rows() -> ObservedTree {
    tree(
        vec![
            node(0, evidence("sheet", None), vec![1, 3], &[]),
            node(1, evidence("list", None), vec![2], &[0]),
            node(2, evidence("row", Some("f0000.txt")), Vec::new(), &[0, 0]),
            node(3, evidence("combobox", Some("Where:")), Vec::new(), &[1]),
        ],
        vec![0],
        true,
    )
}

fn first(role: &str) -> (LocatorQuery, LocatorResolveRequest) {
    let query = LocatorQuery {
        identity: crate::IdentityPredicate {
            role: Some(role.into()),
            ..Default::default()
        },
        ..LocatorQuery::default()
    };
    let request = LocatorResolveRequest {
        selection: LocatorSelection::First,
        deadline: crate::Deadline::from_duration(std::time::Duration::from_secs(5)).unwrap(),
        max_raw_depth: 50,
        surface: None,
        materialization: LocatorMaterialization::None,
    };
    (query, request)
}

#[test]
fn first_answers_from_outside_list_rows_without_reading_them() {
    let adapter = PanelAdapter::default();
    let (query, request) = first("combobox");

    let resolution = resolve_query(
        &adapter,
        &query,
        ObservationRoot::Window(&window()),
        &request,
    )
    .expect("a control outside the rows answers --first");

    assert!(resolution.meta.selection_complete);
    assert_eq!(resolution.matches.len(), 1);
    assert_eq!(resolution.matches[0].data.name, "Where:");
    assert_eq!(adapter.passes_without_rows.load(Ordering::SeqCst), 1);
    assert_eq!(
        adapter.passes_with_rows.load(Ordering::SeqCst),
        0,
        "the rows of a list before the match must stay unread"
    );
}

#[test]
fn first_reads_the_rows_when_nothing_outside_them_matches() {
    let adapter = PanelAdapter::default();
    let (query, request) = first("row");

    let resolution = resolve_query(
        &adapter,
        &query,
        ObservationRoot::Window(&window()),
        &request,
    )
    .expect("a match inside the rows still answers --first");

    assert!(resolution.meta.selection_complete);
    assert_eq!(resolution.matches[0].data.name, "f0000.txt");
    assert_eq!(adapter.passes_without_rows.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.passes_with_rows.load(Ordering::SeqCst), 1);
}

#[test]
fn a_tree_read_without_rows_answers_only_first() {
    for selection in [
        LocatorSelection::All { limit: Some(50) },
        LocatorSelection::Count,
        LocatorSelection::Last,
    ] {
        let tree = panel_without_rows();
        let (query, request) = first("combobox");
        let request = LocatorResolveRequest {
            selection,
            ..request
        };
        let resolution = super::evaluate_locator_tree(tree, &query, &request).unwrap();
        assert!(
            !resolution.meta.selection_complete,
            "{selection:?} needs the rows and must not answer from a tree without them"
        );
    }
}
