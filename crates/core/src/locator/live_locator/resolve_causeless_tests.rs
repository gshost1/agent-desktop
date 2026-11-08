use super::resolve_query_tests::{CountingAdapter, observed_tree, request, request_with_timeout};
use super::test_support::window;
use super::{
    LocatorMaterialization, LocatorResolveRequest, LocatorSelection, LocatorStats,
    ObservationRequest, ObservationRoot, ObservedTree, resolve_query,
};
use crate::{AppError, locator::LocatorQuery};
use std::sync::atomic::{AtomicUsize, Ordering};

fn causeless_then_complete(attempt: usize) -> ObservedTree {
    observed_tree(attempt > 0, LocatorStats::default())
}

#[test]
fn an_incomplete_walk_with_no_recorded_cause_is_walked_again() {
    let adapter = CountingAdapter {
        builds: AtomicUsize::new(0),
        observe: Some(causeless_then_complete),
        observe_with_request: None,
    };

    let resolution = resolve_query(
        &adapter,
        &LocatorQuery::default(),
        ObservationRoot::Window(&window()),
        &request(),
    )
    .expect("an incompleteness with no recorded cause must not end the resolution");

    assert!(resolution.meta.selection_complete);
    assert_eq!(adapter.builds.load(Ordering::SeqCst), 2);
    assert_eq!(resolution.stats.reads.counts.observation_attempts, 2);
}

fn invalidated_twice_then_stable(attempt: usize) -> ObservedTree {
    let mut stats = LocatorStats::default();
    if attempt < 2 {
        stats.reads.health.invalidated_elements = 1;
        observed_tree(false, stats)
    } else {
        observed_tree(true, stats)
    }
}

#[test]
fn elements_destroyed_during_a_walk_are_transient_incompleteness() {
    let adapter = CountingAdapter {
        builds: AtomicUsize::new(0),
        observe: Some(invalidated_twice_then_stable),
        observe_with_request: None,
    };

    let resolution = resolve_query(
        &adapter,
        &LocatorQuery::default(),
        ObservationRoot::Window(&window()),
        &request(),
    )
    .expect("a walk the interface changed under must be retried while time remains");

    assert!(resolution.meta.complete);
    assert_eq!(adapter.builds.load(Ordering::SeqCst), 3);
    assert_eq!(resolution.stats.reads.health.invalidated_elements, 2);
}

fn causeless_after_spending_its_budget(_: usize, request: &ObservationRequest) -> ObservedTree {
    crate::wait::deadline::sleep(request.deadline.remaining());
    observed_tree(false, LocatorStats::default())
}

#[test]
fn a_causeless_walk_that_spent_its_budget_fails_closed_with_its_own_evidence() {
    let clock = crate::wait::deadline::VirtualClock::freeze();
    let adapter = CountingAdapter {
        builds: AtomicUsize::new(0),
        observe: None,
        observe_with_request: Some(causeless_after_spending_its_budget),
    };
    let request = LocatorResolveRequest {
        selection: LocatorSelection::Strict,
        materialization: LocatorMaterialization::SelectedMatches,
        ..request_with_timeout(std::time::Duration::from_secs(5))
    };

    let error = resolve_query(
        &adapter,
        &LocatorQuery::default(),
        ObservationRoot::Window(&window()),
        &request,
    )
    .err()
    .expect("an incomplete walk must not succeed");

    let AppError::Adapter(error) = error else {
        panic!("expected adapter error");
    };
    let details = error.details.unwrap();
    assert_eq!(details["kind"], "locator_incomplete");
    assert_eq!(details["retryable"], false);
    assert_eq!(
        adapter.builds.load(Ordering::SeqCst),
        1,
        "a walk that spent its observation budget must not spend the hydration reserve on another"
    );
    assert_eq!(clock.elapsed(), std::time::Duration::from_secs(4));
}
