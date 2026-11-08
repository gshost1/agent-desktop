use super::MovedLandings;
use crate::refs::resolve_verdict::ResolveVerdict;

#[test]
fn the_third_consecutive_moved_landing_ends_as_saved_ref_absent() {
    let entry = crate::adapter::minimal_ref_entry("button", Some("Run"));
    let mut moved = MovedLandings::default();

    assert!(moved.exhausted(ResolveVerdict::Moved, &entry).is_none());
    assert!(moved.exhausted(ResolveVerdict::Moved, &entry).is_none());
    let refusal = moved
        .exhausted(ResolveVerdict::Moved, &entry)
        .expect("the third moved landing stops the action");

    assert_eq!(refusal.code, crate::ErrorCode::StaleRef);
    assert!(!refusal.permits_retry_by_default());
    assert_eq!(
        refusal.details.expect("details")["kind"],
        "saved_ref_absent"
    );
}

#[test]
fn another_refusal_resets_the_count() {
    let entry = crate::adapter::minimal_ref_entry("button", Some("Run"));
    let mut moved = MovedLandings::default();

    assert!(moved.exhausted(ResolveVerdict::Moved, &entry).is_none());
    assert!(moved.exhausted(ResolveVerdict::Moved, &entry).is_none());
    assert!(
        moved
            .exhausted(ResolveVerdict::Incomplete, &entry)
            .is_none()
    );
    assert!(moved.exhausted(ResolveVerdict::Moved, &entry).is_none());
    assert!(moved.exhausted(ResolveVerdict::Moved, &entry).is_none());
}
