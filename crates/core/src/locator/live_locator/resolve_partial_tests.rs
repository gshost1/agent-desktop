use super::test_support::{evidence, node, tree, window};
use super::{
    IdentifierEvidence, LocatorMaterialization, LocatorResolveRequest, LocatorSelection,
    LocatorStats, ObservationRequest, ObservationRoot, ObservedTree,
    resolve_query_allowing_partial,
};
use crate::{
    AdapterError,
    adapter::{ActionOps, InputOps, ObservationOps, SystemOps},
    locator::LocatorQuery,
};
use std::sync::atomic::{AtomicUsize, Ordering};

struct BudgetCutAdapter {
    builds: AtomicUsize,
}

impl ObservationOps for BudgetCutAdapter {
    fn observe_tree(
        &self,
        _root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        self.builds.fetch_add(1, Ordering::SeqCst);
        crate::wait::deadline::sleep(request.deadline.remaining());
        let mut cancel = evidence("button", Some("Cancel"));
        cancel.identifiers =
            IdentifierEvidence::absent().with_retained_object("axtok1:cancel".into());
        let mut observed = tree(
            vec![
                node(0, evidence("window", Some("Fixture")), vec![1, 2], &[]),
                node(1, cancel, Vec::new(), &[0]),
                node(2, evidence("button", Some("Save")), Vec::new(), &[1]),
            ],
            vec![0],
            false,
        );
        let mut stats = LocatorStats::default();
        stats.reads.health.deadline_exhausted = 1;
        observed.stats = stats;
        Ok(observed)
    }
}

impl ActionOps for BudgetCutAdapter {}
impl InputOps for BudgetCutAdapter {}
impl SystemOps for BudgetCutAdapter {}

#[test]
fn a_walk_the_budget_cuts_answers_with_what_it_saw_when_partial_is_allowed() {
    let _clock = crate::wait::deadline::VirtualClock::freeze();
    let adapter = BudgetCutAdapter {
        builds: AtomicUsize::new(0),
    };
    let request = LocatorResolveRequest {
        selection: LocatorSelection::All { limit: Some(50) },
        deadline: crate::Deadline::from_duration(std::time::Duration::from_secs(5)).unwrap(),
        max_raw_depth: 50,
        surface: None,
        materialization: LocatorMaterialization::SelectedMatches,
    };
    let query = LocatorQuery {
        identity: crate::IdentityPredicate {
            role: Some("button".into()),
            ..Default::default()
        },
        ..LocatorQuery::default()
    };

    let resolution = resolve_query_allowing_partial(
        &adapter,
        &query,
        ObservationRoot::Window(&window()),
        &request,
    )
    .expect("an allowed partial answer is not a timeout");

    assert!(!resolution.meta.selection_complete);
    assert_eq!(resolution.meta.total_matches, 2);
    let refs: Vec<_> = resolution
        .matches
        .iter()
        .map(|found| (found.data.name.as_str(), found.data.ref_id.is_some()))
        .collect();
    assert_eq!(refs, [("Cancel", true), ("Save", false)]);
    assert_eq!(adapter.builds.load(Ordering::SeqCst), 1);
    assert_eq!(resolution.stats.elapsed_us, 4_000_000);
}
