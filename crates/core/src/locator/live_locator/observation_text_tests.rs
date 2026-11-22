use super::{LocatorResolveRequest, ObservationRequest};
use crate::{Deadline, IdentityPredicate, LocatorMaterialization, LocatorQuery, LocatorSelection};

#[test]
fn only_snapshot_requests_collect_contents_index_evidence() {
    let deadline = Deadline::after(500).unwrap();
    assert!(
        ObservationRequest::snapshot(&Default::default(), deadline)
            .observation_mode
            .contents_index
    );
    let request = LocatorResolveRequest {
        selection: LocatorSelection::All { limit: None },
        deadline,
        max_raw_depth: 50,
        materialization: LocatorMaterialization::FullRefMap,
        surface: None,
    };
    let observation = ObservationRequest::locator(&LocatorQuery::default(), &request, deadline);
    assert_eq!(
        observation.evidence_for_raw_depth(0),
        super::EvidenceRequirements::snapshot()
    );
    assert!(!observation.observation_mode.contents_index);
}

#[test]
fn only_text_queries_request_content_not_plain_capture_or_role_queries() {
    let deadline = Deadline::after(500).unwrap();
    assert!(
        !ObservationRequest::snapshot(&Default::default(), deadline)
            .observation_mode
            .text_content
    );
    let request = LocatorResolveRequest {
        selection: LocatorSelection::All { limit: Some(10) },
        deadline,
        max_raw_depth: 50,
        materialization: LocatorMaterialization::SelectedMatches,
        surface: None,
    };
    let mut query = LocatorQuery {
        identity: IdentityPredicate {
            role: Some("paragraph".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(
        !ObservationRequest::locator(&query, &request, deadline)
            .observation_mode
            .text_content
    );
    query.has_text = Some("needle".into());
    assert!(
        ObservationRequest::locator(&query, &request, deadline)
            .observation_mode
            .text_content
    );
    let mut nested = LocatorQuery::default();
    nested.containment.has_not = Some(Box::new(query));
    assert!(
        ObservationRequest::locator(&nested, &request, deadline)
            .observation_mode
            .text_content
    );
}
