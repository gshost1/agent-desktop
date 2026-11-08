//! What a failed saved-ref resolution means for a caller that may try again.
//!
//! A resolver stamps its refusal with an error code and two flags in
//! `details`: `retryable` says whether trying again can change the answer, and
//! `complete` says whether the answer was read over complete evidence. Every
//! loop that re-resolves a ref (the read-only wait before the interaction
//! lease, the dispatch-time retry under it, and `wait --element`) reads that
//! triple through [`ResolveVerdict::of`] alone, so they cannot disagree.
//!
//! Precedence: an explicit `retryable: true` wins over `complete: true`. A
//! complete read that found no unique match (a removed or twinned element, or
//! a stored path that moved under the descent) still retries when its producer
//! says the element may come back; `complete` only decides what a wait reports
//! once its budget runs out. A refusal without `retryable: true` is settled.

use serde_json::Value;

use crate::{AdapterError, ErrorCode};

/// The retry disposition of one resolution refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResolveVerdict {
    /// Trying again cannot change the answer: stop and report it now.
    Settled,
    /// A complete read found no unique live match, but the producer says the
    /// element may come back: retry within the budget, and report this answer
    /// if the budget runs out first.
    Unsettled,
    /// The read was incomplete or cut short: retry within the budget, and
    /// report the wait's own timeout if it runs out first.
    Incomplete,
}

impl ResolveVerdict {
    /// Derives the verdict from the refusal's retryable stamp and its
    /// `complete` flag, under the precedence the module documents.
    pub(crate) fn of(error: &AdapterError) -> Self {
        if !error.is_explicitly_retryable() {
            return Self::Settled;
        }
        let complete = error
            .details
            .as_ref()
            .and_then(|details| details.get("complete"))
            .and_then(Value::as_bool);
        match complete {
            Some(true) => Self::Unsettled,
            _ => Self::Incomplete,
        }
    }

    /// The verdict a ref action's wait acts on. It retries only the refusals a
    /// resolver raises about its target (`STALE_REF`, `AMBIGUOUS_TARGET`,
    /// `TIMEOUT`, `APP_UNRESPONSIVE`); any other code is settled for it,
    /// whatever its stamp.
    pub(crate) fn of_ref_action(error: &AdapterError) -> Self {
        let resolution_refusal = matches!(
            error.code,
            ErrorCode::StaleRef
                | ErrorCode::AmbiguousTarget
                | ErrorCode::Timeout
                | ErrorCode::AppUnresponsive
        );
        if resolution_refusal {
            Self::of(error)
        } else {
            Self::Settled
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
