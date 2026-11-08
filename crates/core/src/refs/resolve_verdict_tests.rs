use serde_json::json;

use super::ResolveVerdict;
use crate::{AdapterError, ErrorCode};

fn stale(details: serde_json::Value) -> AdapterError {
    AdapterError::stale_ref_because("No candidate").with_details(details)
}

#[test]
fn an_explicit_retryable_stamp_beats_a_complete_read() {
    let error = stale(json!({ "complete": true, "retryable": true }));

    assert_eq!(ResolveVerdict::of(&error), ResolveVerdict::Unsettled);
    assert!(ResolveVerdict::of(&error).retries());
}

#[test]
fn a_complete_read_without_a_retryable_stamp_is_settled() {
    for details in [
        json!({ "complete": true, "retryable": false }),
        json!({ "complete": true }),
    ] {
        let verdict = ResolveVerdict::of(&stale(details));

        assert_eq!(verdict, ResolveVerdict::Settled);
        assert!(!verdict.retries());
    }
}

#[test]
fn a_retryable_read_that_was_not_complete_is_incomplete() {
    for details in [
        json!({ "complete": false, "retryable": true }),
        json!({ "retryable": true }),
    ] {
        assert_eq!(
            ResolveVerdict::of(&stale(details)),
            ResolveVerdict::Incomplete
        );
    }
}

#[test]
fn an_unstamped_refusal_is_settled() {
    assert_eq!(
        ResolveVerdict::of(&AdapterError::stale_ref("@e1")),
        ResolveVerdict::Settled
    );
}

#[test]
fn a_ref_action_retries_only_resolution_refusals() {
    let retryable = json!({ "complete": true, "retryable": true });
    for code in [
        ErrorCode::StaleRef,
        ErrorCode::AmbiguousTarget,
        ErrorCode::Timeout,
        ErrorCode::AppUnresponsive,
    ] {
        let error = AdapterError::new(code, "refused").with_details(retryable.clone());
        assert_eq!(
            ResolveVerdict::of_ref_action(&error),
            ResolveVerdict::Unsettled
        );
    }
    for code in [
        ErrorCode::ElementNotFound,
        ErrorCode::AppNotFound,
        ErrorCode::ActionFailed,
    ] {
        let error = AdapterError::new(code, "refused").with_details(retryable.clone());
        assert_eq!(ResolveVerdict::of(&error), ResolveVerdict::Unsettled);
        assert_eq!(
            ResolveVerdict::of_ref_action(&error),
            ResolveVerdict::Settled
        );
    }
}

/// The core refusals keep the disposition the JSON contract documents for
/// them: a moved landing retries, a complete absence and a live twin do not.
#[test]
fn the_core_refusals_carry_their_documented_verdicts() {
    let entry = crate::adapter::minimal_ref_entry("button", Some("Run"));

    assert_eq!(
        ResolveVerdict::of(&crate::refs::resolve_errors::landing_moved(&entry)),
        ResolveVerdict::Unsettled
    );
    assert_eq!(
        ResolveVerdict::of(&crate::refs::resolve_errors::absent(&entry)),
        ResolveVerdict::Settled
    );
    assert_eq!(
        ResolveVerdict::of(&crate::refs::resolve_errors::ambiguous_live_match(&entry)),
        ResolveVerdict::Settled
    );
    assert_eq!(
        ResolveVerdict::of(&crate::refs::resolve_errors::identity_unknown_error(&entry)),
        ResolveVerdict::Incomplete
    );
}
