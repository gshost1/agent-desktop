use crate::{AdapterError, RefEntry, refs::resolve_verdict::ResolveVerdict};

/// How many times one ref action retries a moved landing in a row before it
/// stops: a path that keeps landing a different element is not a list that is
/// settling, so the action gives a stale-ref diagnosis instead of spending its
/// whole budget on a generic timeout.
const MOVED_LANDING_RETRIES: u8 = 2;

/// The consecutive moved landings one ref action has seen, across the
/// read-only wait and the dispatch-time retry under the lease. Any other
/// resolution refusal resets it. A successful provisional landing does not:
/// it skips the whole-window count, so it says nothing about the race.
#[derive(Debug, Default)]
pub(crate) struct MovedLandings {
    consecutive: u8,
}

impl MovedLandings {
    /// Notes the verdict of a refusal the action is about to retry. On the
    /// moved landing past the retry limit it returns the refusal the action
    /// ends with: `STALE_REF` `saved_ref_absent`, which is not retryable.
    pub(crate) fn exhausted(
        &mut self,
        verdict: ResolveVerdict,
        entry: &RefEntry,
    ) -> Option<AdapterError> {
        if verdict != ResolveVerdict::Moved {
            self.consecutive = 0;
            return None;
        }
        self.consecutive = self.consecutive.saturating_add(1);
        (self.consecutive > MOVED_LANDING_RETRIES)
            .then(|| crate::refs::resolve_errors::absent(entry))
    }
}

#[cfg(test)]
#[path = "ref_action_moved_landings_tests.rs"]
mod tests;
