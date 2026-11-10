use super::publish_observed_matches;
use crate::{
    adapter::{ActionOps, InputOps, ObservationOps, SystemOps},
    locator::{
        LocatorQuery,
        live_locator::{
            LocatorMatch, LocatorMatchData, LocatorMaterialization, LocatorResolution,
            LocatorResolutionMeta, LocatorResolveRequest, LocatorSelection, LocatorStats,
            ObservedTree, test_support,
        },
    },
};

struct NoAnchorAdapter;

impl ObservationOps for NoAnchorAdapter {}
impl ActionOps for NoAnchorAdapter {}
impl InputOps for NoAnchorAdapter {}
impl SystemOps for NoAnchorAdapter {}

fn publish(resolution: &mut LocatorResolution) {
    let request = LocatorResolveRequest {
        selection: LocatorSelection::All { limit: Some(50) },
        deadline: crate::Deadline::after(5_000).unwrap(),
        max_raw_depth: 50,
        surface: None,
        materialization: LocatorMaterialization::SelectedMatches,
    };
    publish_observed_matches(
        &NoAnchorAdapter,
        &LocatorQuery::default(),
        &request,
        resolution,
        &incomplete_walk(),
    )
    .unwrap();
}

fn incomplete_walk() -> ObservedTree {
    test_support::tree(
        vec![test_support::node(
            0,
            test_support::evidence("button", Some("Cancel")),
            Vec::new(),
            &[],
        )],
        vec![0],
        false,
    )
}

fn observed(name: &str, token: Option<&str>, fully_observed: bool) -> LocatorMatch {
    let mut entry = crate::adapter::minimal_ref_entry("button", Some(name));
    entry.identity.retained_object = token.map(String::from);
    entry.identity.capture = crate::CaptureVerdict::Ambiguous;
    LocatorMatch {
        data: LocatorMatchData {
            ref_id: Some("@e9".into()),
            role: "button".into(),
            name: name.into(),
            value: None,
            states: Vec::new(),
            interactive: true,
            path: Vec::new(),
        },
        document_order: 0,
        entry,
        screen_bounds: None,
        fully_observed,
    }
}

fn incomplete(matches: Vec<LocatorMatch>) -> LocatorResolution {
    LocatorResolution {
        matches,
        refmap: None,
        stats: LocatorStats::default(),
        meta: LocatorResolutionMeta {
            total_matches: 3,
            complete: false,
            selection_complete: false,
            truncated: false,
            roles_present: Vec::new(),
        },
    }
}

#[test]
fn an_incomplete_walk_publishes_refs_only_for_element_tokens_it_read_whole() {
    let mut resolution = incomplete(vec![
        observed("Cancel", Some("axtok1:cancel"), true),
        observed("Save", None, true),
        observed("Where:", Some("axtok1:where"), false),
    ]);

    publish(&mut resolution);

    let refs: Vec<_> = resolution
        .matches
        .iter()
        .map(|found| found.data.ref_id.is_some())
        .collect();
    assert_eq!(
        refs,
        [true, false, false],
        "only a token may publish a ref, and only once hydration settles the match"
    );
    assert_eq!(
        resolution.refmap.as_ref().map(crate::refs::RefMap::len),
        Some(1)
    );
}

#[test]
fn an_incomplete_walk_without_tokens_publishes_no_refmap() {
    let mut resolution = incomplete(vec![observed("Save", None, true)]);

    publish(&mut resolution);

    assert!(resolution.matches[0].data.ref_id.is_none());
    assert!(resolution.refmap.is_none());
}
