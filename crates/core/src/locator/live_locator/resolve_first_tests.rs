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
    combobox_token: Option<&'static str>,
    pass_without_rows_takes: Option<std::time::Duration>,
    pass_with_rows_fails: Option<crate::ErrorCode>,
    pass_with_rows_cut: bool,
    every_pass_cut: bool,
}

impl PanelAdapter {
    fn stamped(&self, mut tree: ObservedTree) -> ObservedTree {
        if let Some(token) = self.combobox_token {
            for node in &mut tree.nodes {
                if node.evidence.role.known().map(String::as_str) == Some("combobox") {
                    let identifiers = std::mem::replace(
                        &mut node.evidence.identifiers,
                        crate::IdentifierEvidence::absent(),
                    );
                    node.evidence.identifiers = identifiers.with_retained_object(token.to_string());
                }
            }
        }
        tree
    }
}

impl ObservationOps for PanelAdapter {
    fn observe_tree(
        &self,
        _root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        if self.every_pass_cut {
            self.passes_with_rows.fetch_add(1, Ordering::SeqCst);
            return Ok(self.stamped(panel_cut_inside_rows()));
        }
        if request.observation_mode.leave_rows_unread {
            self.passes_without_rows.fetch_add(1, Ordering::SeqCst);
            if let Some(duration) = self.pass_without_rows_takes {
                crate::wait::deadline::sleep(duration);
            }
            return Ok(self.stamped(panel_without_rows()));
        }
        self.passes_with_rows.fetch_add(1, Ordering::SeqCst);
        if let Some(code) = self.pass_with_rows_fails.clone() {
            return Err(AdapterError::new(code, "row read failed"));
        }
        if self.pass_with_rows_cut {
            return Ok(self.stamped(panel_cut_inside_rows()));
        }
        Ok(self.stamped(panel_with_rows()))
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

/// The combobox comes before the list, and the read stopped inside the
/// rows: complete up to the match, not a whole window, and no rows left
/// unread on purpose.
fn panel_cut_inside_rows() -> ObservedTree {
    let mut list = node(2, evidence("list", None), Vec::new(), &[1]);
    list.completeness.subtree_complete = false;
    list.children_count = Some(ROWS);
    let mut root = node(0, evidence("sheet", None), vec![1, 2], &[]);
    root.completeness.subtree_complete = false;
    tree(
        vec![
            root,
            node(1, evidence("combobox", Some("Where:")), Vec::new(), &[0]),
            list,
        ],
        vec![0],
        false,
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

fn first_with_refs(role: &str) -> (LocatorQuery, LocatorResolveRequest) {
    let (query, request) = first(role);
    let request = LocatorResolveRequest {
        materialization: LocatorMaterialization::SelectedMatches,
        ..request
    };
    (query, request)
}

#[test]
fn a_tokenless_first_match_outside_the_rows_waits_for_the_row_pass_to_prove_its_ref() {
    let adapter = PanelAdapter::default();
    let (query, request) = first_with_refs("combobox");

    let resolution = resolve_query(
        &adapter,
        &query,
        ObservationRoot::Window(&window()),
        &request,
    )
    .expect("a control outside the rows answers --first");

    let ref_id = resolution.matches[0]
        .data
        .ref_id
        .as_deref()
        .expect("the match publishes a ref");
    let entry = resolution
        .refmap
        .as_ref()
        .and_then(|refmap| refmap.get(ref_id))
        .expect("the ref is in the published refmap");
    assert!(
        entry.identity.capture.is_unique(),
        "a ref with no element token must be proven over the whole window, got {:?}",
        entry.identity.capture
    );
    assert_eq!(adapter.passes_without_rows.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.passes_with_rows.load(Ordering::SeqCst), 1);
}

fn assert_answered_without_a_ref(resolution: &super::LocatorResolution) {
    assert!(resolution.meta.selection_complete);
    assert_eq!(resolution.matches.len(), 1);
    assert_eq!(resolution.matches[0].data.name, "Where:");
    assert!(
        resolution.matches[0].data.ref_id.is_none(),
        "an unproven tokenless ref must never be published"
    );
    assert!(
        resolution.refmap.is_none(),
        "an answer with no ref must publish no refmap, or find saves an empty snapshot"
    );
}

#[test]
fn a_tokenless_first_match_with_no_budget_left_for_the_rows_is_answered_without_a_ref() {
    let _clock = crate::wait::deadline::VirtualClock::freeze();
    let adapter = PanelAdapter {
        pass_without_rows_takes: Some(std::time::Duration::from_millis(250)),
        ..PanelAdapter::default()
    };
    let (query, request) = first_with_refs("combobox");
    let request = LocatorResolveRequest {
        deadline: crate::Deadline::from_duration(std::time::Duration::from_millis(400)).unwrap(),
        ..request
    };

    let resolution = resolve_query(
        &adapter,
        &query,
        ObservationRoot::Window(&window()),
        &request,
    )
    .expect("the match found before the budget ran out is still the answer");

    assert_answered_without_a_ref(&resolution);
    assert_eq!(adapter.passes_with_rows.load(Ordering::SeqCst), 0);
}

#[test]
fn a_tokenless_first_match_whose_row_pass_times_out_or_stalls_is_answered_without_a_ref() {
    for code in [crate::ErrorCode::Timeout, crate::ErrorCode::AppUnresponsive] {
        let adapter = PanelAdapter {
            pass_with_rows_fails: Some(code.clone()),
            ..PanelAdapter::default()
        };
        let (query, request) = first_with_refs("combobox");

        let resolution = resolve_query(
            &adapter,
            &query,
            ObservationRoot::Window(&window()),
            &request,
        )
        .unwrap_or_else(|error| panic!("{code:?} in the row pass discarded the match: {error:?}"));

        assert_answered_without_a_ref(&resolution);
        assert!(adapter.passes_with_rows.load(Ordering::SeqCst) >= 1);
    }
}

#[test]
fn a_first_match_with_an_element_token_keeps_the_rows_unread() {
    let adapter = PanelAdapter {
        combobox_token: Some("axtok1:combobox"),
        ..PanelAdapter::default()
    };
    let (query, request) = first_with_refs("combobox");

    let resolution = resolve_query(
        &adapter,
        &query,
        ObservationRoot::Window(&window()),
        &request,
    )
    .expect("a control outside the rows answers --first");

    assert!(resolution.matches[0].data.ref_id.is_some());
    assert_eq!(adapter.passes_without_rows.load(Ordering::SeqCst), 1);
    assert_eq!(
        adapter.passes_with_rows.load(Ordering::SeqCst),
        0,
        "a ref that re-identifies itself by token needs no window-wide count"
    );
}

#[test]
fn a_row_pass_that_fails_for_another_reason_returns_its_error() {
    let adapter = PanelAdapter {
        pass_with_rows_fails: Some(crate::ErrorCode::PermDenied),
        ..PanelAdapter::default()
    };
    let (query, request) = first_with_refs("combobox");

    let error = resolve_query(
        &adapter,
        &query,
        ObservationRoot::Window(&window()),
        &request,
    )
    .err()
    .expect("a permission failure is not hidden behind the earlier answer");

    assert_eq!(error.code(), "PERM_DENIED");
}

#[test]
fn a_row_pass_cut_inside_the_rows_publishes_no_unproven_ref() {
    let adapter = PanelAdapter {
        pass_with_rows_cut: true,
        ..PanelAdapter::default()
    };
    let (query, request) = first_with_refs("combobox");

    let resolution = resolve_query(
        &adapter,
        &query,
        ObservationRoot::Window(&window()),
        &request,
    )
    .expect("the match outside the rows still answers --first");

    assert_answered_without_a_ref(&resolution);
}

#[test]
fn a_platform_that_never_leaves_rows_unread_keeps_its_first_answer_as_before() {
    let adapter = PanelAdapter {
        every_pass_cut: true,
        ..PanelAdapter::default()
    };
    let (query, request) = first_with_refs("combobox");

    let resolution = resolve_query(
        &adapter,
        &query,
        ObservationRoot::Window(&window()),
        &request,
    )
    .expect("a cut read answers --first as it did before");

    assert!(resolution.matches[0].data.ref_id.is_some());
    assert_eq!(adapter.passes_with_rows.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.passes_without_rows.load(Ordering::SeqCst), 0);
}
