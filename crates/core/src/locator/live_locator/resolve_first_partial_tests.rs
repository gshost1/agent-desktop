use super::test_support::{evidence, node, tree, window};
use super::{
    LocatorMaterialization, LocatorResolveRequest, LocatorSelection, LocatorStats,
    ObservationRequest, ObservationRoot, ObservedTree, resolve_query,
    resolve_query_allowing_partial,
};
use crate::{
    AdapterError,
    adapter::{ActionOps, InputOps, ObservationOps, SystemOps},
    locator::LocatorQuery,
};
use std::sync::atomic::{AtomicUsize, Ordering};

const ROWS: u32 = 3000;

/// A sheet whose combobox sits outside its list rows. The pass that leaves
/// the rows unread finds the combobox; the pass that reads them stops inside
/// the rows before it reaches the combobox, so it has no answer of its own.
struct RowPassCutAdapter {
    row_passes: AtomicUsize,
    transient: bool,
}

impl ObservationOps for RowPassCutAdapter {
    fn observe_tree(
        &self,
        _root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        if request.observation_mode.leave_rows_unread {
            return Ok(sheet_without_rows());
        }
        self.row_passes.fetch_add(1, Ordering::SeqCst);
        let mut cut = sheet_cut_before_the_combobox();
        if self.transient {
            let mut stats = LocatorStats::default();
            stats.reads.health.deadline_exhausted = 1;
            cut.stats = stats;
        }
        Ok(cut)
    }
}

impl ActionOps for RowPassCutAdapter {}
impl InputOps for RowPassCutAdapter {}
impl SystemOps for RowPassCutAdapter {}

fn unread_list(index: u32, path: &[usize]) -> super::ObservedNode {
    let mut list = node(index, evidence("list", None), Vec::new(), path);
    list.completeness.subtree_complete = false;
    list.children_count = Some(ROWS);
    list
}

fn sheet_without_rows() -> ObservedTree {
    let mut root = node(0, evidence("sheet", None), vec![1, 2], &[]);
    root.completeness.subtree_complete = false;
    let mut sheet = tree(
        vec![
            root,
            unread_list(1, &[0]),
            node(2, evidence("combobox", Some("Where:")), Vec::new(), &[1]),
        ],
        vec![0],
        false,
    );
    sheet.mark_rows_unread();
    sheet
}

fn sheet_cut_before_the_combobox() -> ObservedTree {
    let mut root = node(0, evidence("sheet", None), vec![1], &[]);
    root.completeness.subtree_complete = false;
    tree(vec![root, unread_list(1, &[0])], vec![0], false)
}

fn first_combobox() -> (LocatorQuery, LocatorResolveRequest) {
    let query = LocatorQuery {
        identity: crate::IdentityPredicate {
            role: Some("combobox".into()),
            ..Default::default()
        },
        ..LocatorQuery::default()
    };
    let request = LocatorResolveRequest {
        selection: LocatorSelection::First,
        deadline: crate::Deadline::from_duration(std::time::Duration::from_secs(5)).unwrap(),
        max_raw_depth: 50,
        surface: None,
        materialization: LocatorMaterialization::SelectedMatches,
    };
    (query, request)
}

fn assert_first_pass_answer(resolution: &super::LocatorResolution, mode: &str) {
    assert!(
        resolution.meta.selection_complete,
        "{mode}: the first pass answered --first, so the answer is complete"
    );
    let names: Vec<_> = resolution
        .matches
        .iter()
        .map(|found| (found.data.name.as_str(), found.data.ref_id.is_some()))
        .collect();
    assert_eq!(
        names,
        [("Where:", false)],
        "{mode}: the first pass's match must survive a row pass that cannot answer"
    );
}

/// `--allow-partial` promises that an answer it marks complete is the one the
/// default contract gives. The default keeps the first pass's match when the
/// row pass that should prove its ref cannot finish; the partial path must
/// keep it too, instead of returning the row pass's empty observation.
#[test]
fn an_unfinished_row_pass_keeps_the_first_pass_match_with_or_without_partial() {
    let _clock = crate::wait::deadline::VirtualClock::freeze();
    for transient in [false, true] {
        let (query, request) = first_combobox();
        let strict = RowPassCutAdapter {
            row_passes: AtomicUsize::new(0),
            transient,
        };
        let resolution = resolve_query(
            &strict,
            &query,
            ObservationRoot::Window(&window()),
            &request,
        )
        .unwrap_or_else(|error| panic!("default, transient={transient}: {error:?}"));
        assert_first_pass_answer(&resolution, &format!("default, transient={transient}"));

        let (query, request) = first_combobox();
        let partial = RowPassCutAdapter {
            row_passes: AtomicUsize::new(0),
            transient,
        };
        let resolution = resolve_query_allowing_partial(
            &partial,
            &query,
            ObservationRoot::Window(&window()),
            &request,
        )
        .unwrap_or_else(|error| panic!("partial, transient={transient}: {error:?}"));
        assert_first_pass_answer(&resolution, &format!("partial, transient={transient}"));
        assert!(partial.row_passes.load(Ordering::SeqCst) >= 1);
    }
}
