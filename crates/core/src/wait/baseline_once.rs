//! The window baseline of one command, taken at its first dispatch attempt.

use std::cell::OnceCell;

use crate::{
    CommandContext, Deadline, PlatformAdapter, RefEntry, wait::window_baseline::WindowBaseline,
};

/// Holds the baseline a command takes just before it first dispatches. A
/// retried dispatch keeps the first one instead of listing windows again
/// while it holds the interaction lease.
#[derive(Default)]
pub(crate) struct BaselineOnce {
    taken: OnceCell<Option<WindowBaseline>>,
}

impl BaselineOnce {
    pub(crate) fn take_before_dispatch(
        &self,
        adapter: &dyn PlatformAdapter,
        entry: &RefEntry,
        context: &CommandContext,
        deadline: Deadline,
    ) {
        self.taken
            .get_or_init(|| WindowBaseline::before_action(adapter, entry, context, deadline));
    }

    pub(crate) fn into_baseline(self) -> Option<WindowBaseline> {
        self.taken.into_inner().flatten()
    }
}

#[cfg(test)]
#[path = "baseline_once_tests.rs"]
mod tests;
