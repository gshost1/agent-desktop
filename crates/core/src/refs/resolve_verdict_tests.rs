use serde_json::json;

use super::ResolveVerdict;
use crate::{AdapterError, ErrorCode};

fn stale(details: serde_json::Value) -> AdapterError {
    AdapterError::stale_ref_because("No candidate").with_details(details)
}

#[test]
fn a_known_complete_absence_is_final_for_a_ref_action_whatever_its_stamp() {
    for kind in ["resolve_no_candidate", "resolution_complete_absence"] {
        let error = stale(json!({ "kind": kind, "complete": true, "retryable": true }));

        assert_eq!(ResolveVerdict::of(&error), ResolveVerdict::Absent);
        assert!(ResolveVerdict::of(&error).retries());
        assert_eq!(
            ResolveVerdict::for_ref_action(&error),
            ResolveVerdict::Settled
        );
    }
}

#[test]
fn an_unknown_complete_retryable_stale_ref_is_unsettled_not_absent() {
    let error =
        stale(json!({ "kind": "resolve_menu_closing", "complete": true, "retryable": true }));

    assert_eq!(ResolveVerdict::of(&error), ResolveVerdict::Unsettled);
    assert_eq!(
        ResolveVerdict::for_ref_action(&error),
        ResolveVerdict::Unsettled
    );
}

#[test]
fn a_retryable_complete_twin_count_is_unsettled() {
    let error = AdapterError::ambiguous_target("2 candidates")
        .with_details(json!({ "complete": true, "retryable": true }));

    assert_eq!(
        ResolveVerdict::for_ref_action(&error),
        ResolveVerdict::Unsettled
    );
}

#[test]
fn a_complete_retryable_timeout_or_unresponsive_read_is_incomplete() {
    for code in [ErrorCode::Timeout, ErrorCode::AppUnresponsive] {
        let error = AdapterError::new(code, "slow")
            .with_details(json!({ "complete": true, "retryable": true }));

        assert_eq!(
            ResolveVerdict::for_ref_action(&error),
            ResolveVerdict::Incomplete
        );
    }
}

#[test]
fn a_refusal_without_a_retryable_stamp_is_settled() {
    for error in [
        stale(json!({ "kind": "resolve_no_candidate", "complete": true, "retryable": false })),
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
        json!({ "kind": "resolve_no_candidate", "complete": false, "retryable": true }),
        json!({ "retryable": true }),
    ] {
        assert_eq!(
            ResolveVerdict::for_ref_action(&stale(details)),
            ResolveVerdict::Incomplete
        );
    }
}

#[test]
fn a_ref_action_retries_only_resolution_refusals_across_every_error_code() {
    let all = [
        ErrorCode::PermDenied,
        ErrorCode::ElementNotFound,
        ErrorCode::AppNotFound,
        ErrorCode::ActionFailed,
        ErrorCode::ActionNotSupported,
        ErrorCode::StaleRef,
        ErrorCode::AmbiguousTarget,
        ErrorCode::WindowNotFound,
        ErrorCode::PlatformNotSupported,
        ErrorCode::Timeout,
        ErrorCode::InvalidArgs,
        ErrorCode::NotificationNotFound,
        ErrorCode::SnapshotNotFound,
        ErrorCode::PolicyDenied,
        ErrorCode::AppUnresponsive,
        ErrorCode::Internal,
    ];
    let retryable = [
        ErrorCode::StaleRef,
        ErrorCode::AmbiguousTarget,
        ErrorCode::Timeout,
        ErrorCode::AppUnresponsive,
    ];

    for code in all {
        let expected = retryable.contains(&code);
        let error = AdapterError::new(code.clone(), "failure")
            .with_details(json!({ "complete": false, "retryable": true }));
        assert!(ResolveVerdict::of(&error).retries());
        assert_eq!(ResolveVerdict::for_ref_action(&error).retries(), expected);
        let unstamped = AdapterError::new(code, "failure");
        assert!(!ResolveVerdict::for_ref_action(&unstamped).retries());
    }
}

/// The core refusals keep the disposition the JSON contract documents for
/// them: a moved landing is counted and retried, a complete absence and a
/// live twin are settled.
#[test]
fn the_core_refusals_carry_their_documented_verdicts() {
    let entry = crate::adapter::minimal_ref_entry("button", Some("Run"));
    let cases = [
        (
            crate::refs::resolve_errors::landing_moved(&entry),
            ResolveVerdict::Moved,
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
