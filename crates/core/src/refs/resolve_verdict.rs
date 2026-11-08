//! What a failed saved-ref resolution means for a ref action that may try
//! again.
//!
//! A resolver stamps its refusal with an error code, a `details.kind`, and two
//! flags: `retryable` says whether trying again can change the answer, and
//! `complete` says whether the answer was read over complete evidence.
//! [`ResolveVerdict::of`] is the one place that reads them, and a ref action
//! (the read-only wait before the interaction lease, the dispatch-time retry
//! under it, and the pointer actions' waits) acts on
//! [`ResolveVerdict::for_ref_action`]:
//!
//! - A refusal without `retryable: true` is settled.
//! - A known complete absence (`resolve_no_candidate`,
//!   `resolution_complete_absence`, `saved_ref_absent`) is final whatever its
//!   `retryable` stamp: the element is not there, and the recovery is a new
//!   snapshot, so the action answers `STALE_REF` at once. That includes a list
//!   rebuilt under the lease while the action resolved it.
//! - A moved landing (`saved_ref_landing_moved`) is retried a bounded number
//!   of times by the caller, which counts it.
//! - Any other retryable `STALE_REF` or `AMBIGUOUS_TARGET` read over complete
//!   evidence is unsettled: retried, and reported if the budget runs out.
//! - Everything else retryable is incomplete: retried, and the wait's own
//!   timeout is reported if the budget runs out.
//!
//! `wait --element` waits for an element to appear, so it polls every refusal
//! stamped `retryable: true`, a complete absence included, and reads that
//! stamp directly: `ResolveVerdict::of(error).retries()` is exactly it.

use serde_json::Value;

use crate::{AdapterError, ErrorCode};

const COMPLETE_ABSENCE_KINDS: [&str; 3] = [
    "resolve_no_candidate",
    "resolution_complete_absence",
    "saved_ref_absent",
];

/// The retry disposition of one resolution refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResolveVerdict {
    /// Trying again cannot change the answer: stop and report it now.
    Settled,
    /// A known complete absence its producer stamped retryable: the element
    /// may be drawn again, but a ref action treats it as final.
    Absent,
    /// The stored path landed a different element after the count found the
    /// ref's one: retried, a bounded number of times.
    Moved,
    /// A retryable `STALE_REF` or `AMBIGUOUS_TARGET` read over complete
    /// evidence: retried, and reported if the budget runs out first.
    Unsettled,
    /// The read was incomplete or cut short: retried, and the wait's own
    /// timeout is reported if the budget runs out first.
    Incomplete,
}

impl ResolveVerdict {
    /// Classifies a refusal from its code, kind, retryable stamp and
    /// `complete` flag.
    pub(crate) fn of(error: &AdapterError) -> Self {
        if !error.is_explicitly_retryable() {
            return Self::Settled;
        }
        let details = error.details.as_ref();
        let kind = details
            .and_then(|details| details.get("kind"))
            .and_then(Value::as_str);
        let complete = details
            .and_then(|details| details.get("complete"))
            .and_then(Value::as_bool)
            == Some(true);
        match (&error.code, kind) {
            (ErrorCode::StaleRef, Some("saved_ref_landing_moved")) => Self::Moved,
            (ErrorCode::StaleRef, Some(kind))
                if complete && COMPLETE_ABSENCE_KINDS.contains(&kind) =>
            {
                Self::Absent
            }
            (ErrorCode::StaleRef | ErrorCode::AmbiguousTarget, _) if complete => Self::Unsettled,
            _ => Self::Incomplete,
        }
    }

    /// The verdict a ref action acts on. A complete absence is final, and only
    /// the refusals a resolver raises about its target (`STALE_REF`,
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
