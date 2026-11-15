use super::*;

#[test]
fn selected_ref_hydration_has_budget_without_extending_the_operation() {
    let request = LocatorResolveRequest {
        selection: super::super::LocatorSelection::First,
        deadline: crate::Deadline::after(60_000).unwrap(),
        max_raw_depth: 50,
        surface: None,
        materialization: super::super::LocatorMaterialization::SelectedMatches,
    };
    let observation = observation_deadline(&request);
    assert!(observation.was_capped());
    assert!(!request.deadline.was_capped());
    assert_eq!(observation.timeout_ms(), request.deadline.timeout_ms());
    let count_request = LocatorResolveRequest {
        materialization: super::super::LocatorMaterialization::None,
        ..request
    };
    assert_eq!(observation_deadline(&count_request), request.deadline);
}
