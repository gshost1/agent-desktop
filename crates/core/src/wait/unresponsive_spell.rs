//! How long a wait keeps polling an application that does not answer.

use std::time::{Duration, Instant};

/// The longest a wait polls an owner that answers no read. An application
/// busy with the work an action started can block for a while and recover,
/// so a single unanswered read never ends the wait; a spell this long ends it
/// with `APP_UNRESPONSIVE` instead of a `TIMEOUT` at the end of the budget.
pub(crate) const UNRESPONSIVE_LIMIT: Duration = Duration::from_secs(5);

/// The current run of consecutive reads that found the owner unresponsive.
#[derive(Debug, Default)]
pub(crate) struct UnresponsiveSpell {
    since: Option<Instant>,
}

impl UnresponsiveSpell {
    /// Records one read. Returns how long the owner has not answered once
    /// that reaches [`UNRESPONSIVE_LIMIT`]; any answered read starts over.
    pub(crate) fn record(&mut self, unresponsive: bool) -> Option<Duration> {
        if !unresponsive {
            self.since = None;
            return None;
        }
        let now = crate::wait::deadline::now();
        let since = *self.since.get_or_insert(now);
        let spell = now.saturating_duration_since(since);
        (spell >= UNRESPONSIVE_LIMIT).then_some(spell)
    }
}

#[cfg(test)]
#[path = "unresponsive_spell_tests.rs"]
mod tests;
