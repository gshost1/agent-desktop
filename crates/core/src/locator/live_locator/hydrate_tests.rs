use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::locator::live_locator::test_support::{evidence, node, tree};
use crate::locator::live_locator::{
    LocatorMaterialization, LocatorResolveRequest, LocatorSelection, evaluate_locator_tree,
};
use crate::{CaptureVerdict, Deadline, NativeHandle};
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn selected_hydration_preserves_window_geometry_and_refuses_unread_or_mixed_frames() {
    use crate::{BoundsSpace, LocatorField, Rect};
    let bounds = Rect {
        x: 10.0,
        y: 20.0,
        width: 80.0,
        height: 30.0,
    };
    let query = LocatorQuery::default();
    let mut observed = node(0, evidence("button", None), vec![], &[]);
    observed.evidence.ref_evidence.bounds = LocatorField::Unknown;
    observed.evidence.ref_evidence.window_bounds = LocatorField::Known(bounds);
    let source = tree(vec![observed.clone()], vec![0], true).source;
    let mut entry = super::super::materialize::ref_entry(&observed, &source, &query);
    assert_eq!(entry.geometry.bounds_space, Some(BoundsSpace::Window));
    assert!(evidence_complete(&query, &observed, BoundsSpace::Window));
    assert!(anchor_matches(&entry, &observed, false));

    observed.evidence.ref_evidence.window_bounds = LocatorField::Known(Rect { x: 110.0, ..bounds });
    observed.evidence.ref_evidence.bounds = LocatorField::Known(bounds);
    assert!(!anchor_matches(&entry, &observed, false));
    observed.evidence.ref_evidence.bounds = LocatorField::Unknown;
    for window in [LocatorField::Unknown, LocatorField::Absent] {
        observed.evidence.ref_evidence.window_bounds = window;
        assert!(!evidence_complete(&query, &observed, BoundsSpace::Window));
        assert!(!anchor_matches(&entry, &observed, false));
    }
    observed.evidence.ref_evidence.window_bounds = LocatorField::Unknown;
    observed.evidence.ref_evidence.bounds = LocatorField::Known(bounds);
    assert!(!evidence_complete(&query, &observed, BoundsSpace::Window));
    observed.evidence.ref_evidence.window_bounds = LocatorField::Known(bounds);
    observed.evidence.ref_evidence.bounds = LocatorField::Unknown;
    entry.geometry.bounds_space = Some(BoundsSpace::Screen);
    assert!(!evidence_complete(&query, &observed, BoundsSpace::Screen));
    assert!(!anchor_matches(&entry, &observed, false));
}

#[test]
fn a_positional_retained_object_is_not_hydration_identity() {
    let mut observed = evidence("textfield", None);
    observed.identifiers = observed.identifiers.with_retained_object("host:1".into());
    let observed = node(0, observed, vec![], &[]);
    let tree = tree(vec![observed.clone()], vec![0], true);
    let entry =
        super::super::materialize::ref_entry(&observed, &tree.source, &LocatorQuery::default());
    assert!(!anchor_matches(&entry, &observed, false));
    let mut replacement = observed;
    replacement.evidence.identifiers = crate::IdentifierEvidence::absent();
    assert!(!anchor_matches(&entry, &replacement, false));
    replacement.evidence.identifiers =
        crate::IdentifierEvidence::absent().with_retained_object("host:2".into());
    assert!(!anchor_matches(&entry, &replacement, false));
}

struct HydrationAdapter {
    reads: AtomicUsize,
}

impl ObservationOps for HydrationAdapter {
    fn resolve_locator_anchor(
        &self,
        _entry: &RefEntry,
        _deadline: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(NativeHandle::null())
    }

    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        _request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        let mut observed = tree(
            vec![node(
                0,
                evidence("button", Some("Replacement")),
                vec![],
                &[],
            )],
            vec![0],
            true,
        );
        observed.source =
            super::super::ObservationSource::from_root(&root, crate::SnapshotSurface::Window);
        Ok(observed)
    }
}

impl ActionOps for HydrationAdapter {}
impl InputOps for HydrationAdapter {}
impl SystemOps for HydrationAdapter {}

fn selected_request() -> LocatorResolveRequest {
    LocatorResolveRequest {
        selection: LocatorSelection::All { limit: None },
        deadline: Deadline::standard().unwrap(),
        max_raw_depth: 50,
        surface: None,
        materialization: LocatorMaterialization::SelectedMatches,
    }
}

fn anchorless_tree(needs_hydration: bool) -> ObservedTree {
    let mut button = evidence("button", None);
    if needs_hydration {
        button.states = super::super::LocatorField::Unknown;
    }
    tree(
        vec![
            node(0, button.clone(), vec![1], &[]),
            node(1, button, vec![], &[0]),
        ],
        vec![0],
        true,
    )
}

#[test]
fn anchorless_fully_observed_matches_preserve_capture_and_refuse_saved_resolution() {
    let observed = anchorless_tree(false);
    let query = LocatorQuery::default();
    let request = selected_request();
    let adapter = HydrationAdapter {
        reads: AtomicUsize::new(0),
    };
    let mut resolution = evaluate_locator_tree(observed.clone(), &query, &request).unwrap();
    assert_eq!(resolution.matches.len(), 2);
    assert!(
        resolution
            .matches
            .iter()
            .all(|matched| matched.fully_observed)
    );

    selected_matches(&adapter, &query, &request, &mut resolution, &observed).unwrap();

    for matched in &resolution.matches {
        assert_eq!(matched.entry.identity.capture, CaptureVerdict::Ambiguous);
        let entry = resolution
            .refmap
            .as_ref()
            .unwrap()
            .get(matched.data.ref_id.as_ref().unwrap())
            .unwrap();
        assert_eq!(entry.identity.capture, CaptureVerdict::Ambiguous);
        let error = crate::refs::resolve_saved_ref::resolve_saved_ref(
            &adapter,
            entry,
            request.deadline,
            None,
        )
        .err()
        .unwrap();
        assert_eq!(error.code, ErrorCode::AmbiguousTarget);
        assert_eq!(error.details.unwrap()["kind"], "identity_not_unique");
    }
    assert_eq!(adapter.reads.load(Ordering::SeqCst), 0);
}

#[test]
fn anchorless_match_needing_hydration_still_refuses_before_reading() {
    let observed = anchorless_tree(true);
    let query = LocatorQuery::default();
    let request = selected_request();
    let adapter = HydrationAdapter {
        reads: AtomicUsize::new(0),
    };
    let mut resolution = evaluate_locator_tree(observed.clone(), &query, &request).unwrap();
    assert!(!resolution.matches[0].fully_observed);

    let error =
        selected_matches(&adapter, &query, &request, &mut resolution, &observed).unwrap_err();

    assert_eq!(error.code(), "STALE_REF");
    let AppError::Adapter(error) = error else {
        panic!("expected adapter error")
    };
    assert_eq!(
        error.details.unwrap()["kind"],
        "locator_selected_anchor_missing"
    );
    assert_eq!(adapter.reads.load(Ordering::SeqCst), 0);
}

#[test]
fn changed_during_selected_hydration_still_reports_staleness() {
    let mut original = evidence("button", Some("Original"));
    original.states = super::super::LocatorField::Unknown;
    let observed = tree(vec![node(0, original, vec![], &[])], vec![0], true);
    let query = LocatorQuery::default();
    let request = selected_request();
    let adapter = HydrationAdapter {
        reads: AtomicUsize::new(0),
    };
    let mut resolution = evaluate_locator_tree(observed.clone(), &query, &request).unwrap();

    let error =
        selected_matches(&adapter, &query, &request, &mut resolution, &observed).unwrap_err();

    assert_eq!(error.code(), "STALE_REF");
    let AppError::Adapter(error) = error else {
        panic!("expected adapter error")
    };
    assert_eq!(error.message, "Selected locator changed during hydration");
    assert_eq!(adapter.reads.load(Ordering::SeqCst), 2);
}

#[test]
fn empty_selection_needs_no_anchor_or_reads() {
    let observed = tree(vec![], vec![], true);
    let query = LocatorQuery::default();
    let request = selected_request();
    let adapter = HydrationAdapter {
        reads: AtomicUsize::new(0),
    };
    let mut resolution = evaluate_locator_tree(observed.clone(), &query, &request).unwrap();

    selected_matches(&adapter, &query, &request, &mut resolution, &observed).unwrap();

    assert!(resolution.matches.is_empty());
    assert_eq!(adapter.reads.load(Ordering::SeqCst), 0);
}
