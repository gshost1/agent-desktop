use super::test_support::{evidence, node, tree, window};
use super::{
    LocatorField, LocatorMaterialization, LocatorResolveRequest, LocatorSelection,
    ObservationRequest, ObservationRoot, ObservedTree, resolve_query,
};
use crate::{
    AdapterError, NativeHandle, Rect,
    adapter::{ActionOps, InputOps, ObservationOps, SystemOps},
    locator::LocatorQuery,
};
use std::sync::Mutex;

/// Reads only what the request asks for at each depth, as an adapter that
/// honors the evidence plan does, and records what it was asked for.
#[derive(Default)]
struct PlanHonoringAdapter {
    descendant_actions_requested: Mutex<Vec<bool>>,
    element_reads: Mutex<usize>,
    anchor_resolutions: Mutex<usize>,
}

fn bounds() -> Rect {
    Rect {
        x: 0.0,
        y: 0.0,
        width: 400.0,
        height: 300.0,
    }
}

fn read(role: &str, name: Option<&str>, full: bool) -> super::LocatorEvidence {
    let mut read = evidence(role, name);
    read.ref_evidence.bounds = LocatorField::Known(bounds());
    read.ref_evidence.window_bounds = LocatorField::Known(bounds());
    if !full {
        read.states = LocatorField::Unknown;
        read.ref_evidence.available_actions = LocatorField::Unknown;
    }
    read
}

impl ObservationOps for PlanHonoringAdapter {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        if matches!(root, ObservationRoot::Element { .. }) {
            *self.element_reads.lock().unwrap() += 1;
            return Err(AdapterError::internal("a root match needs no second read"));
        }
        let root_full = request.evidence_for_raw_depth(0).states;
        let descendants = request.descendant_evidence();
        self.descendant_actions_requested
            .lock()
            .unwrap()
            .push(descendants.ref_evidence.actions);
        Ok(tree(
            vec![
                node(0, read("window", Some("Fixture"), root_full), vec![1], &[]),
                node(1, read("group", None, descendants.states), vec![2], &[0]),
                node(
                    2,
                    read("statictext", Some("needle"), descendants.states),
                    Vec::new(),
                    &[0, 0],
                ),
            ],
            vec![0],
            true,
        ))
    }

    fn resolve_locator_anchor(
        &self,
        _entry: &crate::RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        *self.anchor_resolutions.lock().unwrap() += 1;
        Ok(NativeHandle::null())
    }
}

impl ActionOps for PlanHonoringAdapter {}
impl InputOps for PlanHonoringAdapter {}
impl SystemOps for PlanHonoringAdapter {}

/// `find --text needle --first` answers with the window, the first node in
/// traversal order whose subtree holds the text. Its read carries every field
/// a ref needs, so it publishes from the one window walk, and no other node
/// is asked for its actions or states.
#[test]
fn a_text_first_match_at_the_root_publishes_from_one_walk_without_reading_every_nodes_actions() {
    let adapter = PlanHonoringAdapter::default();
    let query = LocatorQuery {
        has_text: Some("needle".into()),
        ..LocatorQuery::default()
    };
    let request = LocatorResolveRequest {
        selection: LocatorSelection::First,
        deadline: crate::Deadline::from_duration(std::time::Duration::from_secs(5)).unwrap(),
        max_raw_depth: 50,
        surface: None,
        materialization: LocatorMaterialization::SelectedMatches,
    };

    let resolution = resolve_query(
        &adapter,
        &query,
        ObservationRoot::Window(&window()),
        &request,
    )
    .expect("the window holds the text");

    assert!(resolution.meta.selection_complete);
    assert_eq!(resolution.matches.len(), 1);
    assert_eq!(resolution.matches[0].data.role, "window");
    assert_eq!(
        *adapter.descendant_actions_requested.lock().unwrap(),
        [false],
        "one window walk, and it must not read the actions of nodes that publish nothing"
    );
    assert_eq!(*adapter.element_reads.lock().unwrap(), 0);
    assert_eq!(*adapter.anchor_resolutions.lock().unwrap(), 0);
}
