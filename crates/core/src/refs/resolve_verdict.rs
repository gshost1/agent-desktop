//! What a failed saved-ref resolution means for a caller that may try again.
//!
//! A resolver stamps its refusal with an error code and two flags in
//! `details`: `retryable` says whether trying again can change the answer, and
//! `complete` says whether the answer was read over complete evidence.
//! [`ResolveVerdict::of`] is the one place that reads that triple. Each loop
//! that re-resolves a ref applies a named policy to the verdict instead of
//! reading the JSON itself:
//!
//! - A ref action ([`ResolveVerdict::for_ref_action`]: the read-only wait
//!   before the interaction lease and the dispatch-time retry under it) treats
//!   a complete search with no candidate as final, whatever its `retryable`
//!   stamp: the element is not there, and the recovery is a new snapshot, so
//!   the action answers `STALE_REF` at once instead of spending its budget.
//! - `wait --element` ([`ResolveVerdict::of`]) exists to wait for an element
//!   to appear, so it keeps polling any refusal stamped `retryable: true`,
//!   a complete absence included, until its deadline.
//!
//! A refusal without `retryable: true` is settled under both policies.

use serde_json::Value;

use crate::{AdapterError, ErrorCode};

/// The retry disposition of one resolution refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResolveVerdict {
    /// Trying again cannot change the answer: stop and report it now.
    Settled,
    /// A complete search found no candidate, and its producer says the element
    /// may be drawn again. Final for a ref action; a predicate wait polls it.
    Absent,
    /// Any other refusal read over complete evidence and stamped retryable:
    /// retry within the budget, and report this answer if it runs out first.
    Unsettled,
    /// The read was incomplete or cut short: retry within the budget, and
    /// report the wait's own timeout if it runs out first.
    Incomplete,
}

impl ResolveVerdict {
    /// Classifies a refusal from its code, its retryable stamp and its
    /// `complete` flag. This is the verdict `wait --element` acts on.
    pub(crate) fn of(error: &AdapterError) -> Self {
        if !error.is_explicitly_retryable() {
            return Self::Settled;
        }
        let complete = error
            .details
            .as_ref()
            .and_then(|details| details.get("complete"))
            .and_then(Value::as_bool);
        match (complete, &error.code) {
            (Some(true), ErrorCode::StaleRef) => Self::Absent,
            (Some(true), _) => Self::Unsettled,
            _ => Self::Incomplete,
        }
    }

    /// The verdict a ref action's wait acts on. A complete absence is final,
    /// and only the refusals a resolver raises about its target (`STALE_REF`,
    /// `AMBIGUOUS_TARGET`, `TIMEOUT`, `APP_UNRESPONSIVE`) may be retried; any
    /// other code is settled for it, whatever its stamp.
    pub(crate) fn for_ref_action(error: &AdapterError) -> Self {
        let resolution_refusal = matches!(
            error.code,
            ErrorCode::StaleRef
                | ErrorCode::AmbiguousTarget
                | ErrorCode::Timeout
                | ErrorCode::AppUnresponsive
        );
        match Self::of(error) {
            _ if !resolution_refusal => Self::Settled,
            Self::Absent => Self::Settled,
            verdict => verdict,
        }
    }

    /// Whether a caller with budget left tries the resolution again.
    pub(crate) fn retries(self) -> bool {
        !matches!(self, Self::Settled)
    }
}

#[cfg(test)]
#[path = "resolve_verdict_tests.rs"]
mod tests;
