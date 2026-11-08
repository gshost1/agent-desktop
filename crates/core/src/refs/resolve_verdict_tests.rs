use serde_json::json;

use super::ResolveVerdict;
use crate::{AdapterError, ErrorCode};

fn stale(details: serde_json::Value) -> AdapterError {
    AdapterError::stale_ref_because("No candidate").with_details(details)
}

#[test]
fn a_retryable_complete_absence_is_final_for_a_ref_action_but_polled_by_a_wait() {
    let error = stale(json!({ "complete": true, "retryable": true }));

    assert_eq!(ResolveVerdict::of(&error), ResolveVerdict::Absent);
    assert!(ResolveVerdict::of(&error).retries());
    assert_eq!(
        ResolveVerdict::for_ref_action(&error),
        ResolveVerdict::Settled
    );
}

#[test]
fn a_retryable_complete_twin_count_is_unsettled_under_both_policies() {
    let error = AdapterError::ambiguous_target("2 candidates")
        .with_details(json!({ "complete": true, "retryable": true }));

    assert_eq!(ResolveVerdict::of(&error), ResolveVerdict::Unsettled);
    assert_eq!(
        ResolveVerdict::for_ref_action(&error),
        ResolveVerdict::Unsettled
    );
}

#[test]
fn a_refusal_without_a_retryable_stamp_is_settled() {
    for error in [
        stale(json!({ "complete": true, "retryable": false })),
        stale(json!({ "complete": true })),
        AdapterError::stale_ref("@e1"),
    ] {
        assert_eq!(ResolveVerdict::of(&error), ResolveVerdict::Settled);
        assert_eq!(
            ResolveVerdict::for_ref_action(&error),
            ResolveVerdict::Settled
        );
    }
}

#[test]
fn a_retryable_read_that_was_not_complete_is_incomplete() {
    for details in [
        json!({ "complete": false, "retryable": true }),
        json!({ "retryable": true }),
    ] {
        let error = stale(details);
        assert_eq!(ResolveVerdict::of(&error), ResolveVerdict::Incomplete);
        assert_eq!(
            ResolveVerdict::for_ref_action(&error),
            ResolveVerdict::Incomplete
        );
    }
}

#[test]
fn a_ref_action_retries_only_resolution_refusals() {
    let retryable = json!({ "complete": false, "retryable": true });
    for code in [
        ErrorCode::StaleRef,
        ErrorCode::AmbiguousTarget,
        ErrorCode::Timeout,
        ErrorCode::AppUnresponsive,
    ] {
        let error = AdapterError::new(code, "refused").with_details(retryable.clone());
        assert!(ResolveVerdict::for_ref_action(&error).retries());
    }
    for code in [
        ErrorCode::ElementNotFound,
        ErrorCode::AppNotFound,
        ErrorCode::ActionFailed,
    ] {
        let error = AdapterError::new(code, "refused").with_details(retryable.clone());
        assert!(ResolveVerdict::of(&error).retries());
        assert_eq!(
            ResolveVerdict::for_ref_action(&error),
            ResolveVerdict::Settled
        );
    }
}

/// The core refusals keep the disposition the JSON contract documents for
/// them: a moved landing is incomplete evidence and retries, a complete
/// absence and a live twin do not.
#[test]
fn the_core_refusals_carry_their_documented_verdicts() {
    let entry = crate::adapter::minimal_ref_entry("button", Some("Run"));
    let cases = [
        (
            crate::refs::resolve_errors::landing_moved(&entry),
            ResolveVerdict::Incomplete,
        ),
        (
            crate::refs::resolve_errors::absent(&entry),
            ResolveVerdict::Settled,
        ),
        (
            crate::refs::resolve_errors::ambiguous_live_match(&entry),
            ResolveVerdict::Settled,
        ),
        (
            crate::refs::resolve_errors::identity_unknown_error(&entry),
            ResolveVerdict::Incomplete,
        ),
    ];
    for (error, expected) in cases {
        assert_eq!(ResolveVerdict::for_ref_action(&error), expected);
    }
}
