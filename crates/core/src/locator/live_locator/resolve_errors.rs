use super::{LocatorResolution, LocatorStats};
use crate::{AdapterError, ErrorCode};
use serde_json::json;
use std::time::Instant;

pub(super) fn unknown_incomplete_error(resolution: &LocatorResolution) -> AdapterError {
    AdapterError::timeout("Locator traversal returned incomplete evidence")
        .with_suggestion("Retry with a narrower locator or a larger observation budget")
        .with_details(json!({
            "kind": "locator_incomplete",
            "retryable": false,
            "observed_matches": resolution.meta.total_matches,
            "query_stats": resolution.stats,
        }))
        .with_disposition(crate::DeliverySemantics::not_delivered())
}

pub(super) fn has_deterministic_limit(stats: &LocatorStats) -> bool {
    let limits = &stats.traversal.limits;
    limits.node_hits > 0
        || limits.edge_hits > 0
        || limits.child_hits > 0
        || limits.text_hits > 0
        || limits.depth_hits > 0
}

pub(super) fn has_transient_incompleteness(stats: &LocatorStats) -> bool {
    stats.traversal.limits.child_count_changes > 0
        || stats.reads.health.cannot_complete > 0
        || stats.reads.health.deadline_exhausted > 0
        || stats.reads.health.invalidated_elements > 0
}

pub(super) fn deterministic_limit_error(resolution: &LocatorResolution) -> AdapterError {
    AdapterError::new(
        ErrorCode::Timeout,
        "Locator traversal reached a deterministic observation budget",
    )
    .with_suggestion(
        "Use a narrower locator, a shallower scope, or increase the matching observation budget",
    )
    .with_details(json!({
        "kind": "locator_budget_limit",
        "retryable": false,
        "observed_matches": resolution.meta.total_matches,
        "limits": resolution.stats.traversal.limits,
        "query_stats": resolution.stats,
    }))
    .with_disposition(crate::DeliverySemantics::not_delivered())
}

pub(super) fn incomplete_evidence(resolution: &LocatorResolution) -> serde_json::Value {
    json!({
        "observed_matches": resolution.meta.total_matches,
        "child_count_changes": resolution.stats.traversal.limits.child_count_changes,
        "cannot_complete": resolution.stats.reads.health.cannot_complete,
        "deadline_exhausted": resolution.stats.reads.health.deadline_exhausted,
        "invalidated_elements": resolution.stats.reads.health.invalidated_elements,
    })
}

pub(super) fn transient_incomplete_timeout(
    deadline: crate::Deadline,
    stats: &LocatorStats,
    last_incomplete: Option<serde_json::Value>,
    started: Instant,
) -> AdapterError {
    let stats = LocatorStats {
        elapsed_us: super::resolve::elapsed_us_since(started),
        ..stats.clone()
    };
    AdapterError::timeout("Locator observation did not stabilize before its deadline")
        .with_suggestion(
            "Retry with a larger timeout or narrow the locator to a more stable subtree",
        )
        .with_details(json!({
            "kind": "locator_transient_incomplete",
            "retryable": true,
            "timeout_ms": deadline.timeout_ms(),
            "observation_attempts": stats.reads.counts.observation_attempts,
            "last_incomplete": last_incomplete,
            "query_stats": stats,
        }))
        .with_disposition(crate::DeliverySemantics::not_delivered())
}
