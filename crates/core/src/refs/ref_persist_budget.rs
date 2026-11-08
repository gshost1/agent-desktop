use crate::{AppError, Deadline};

/// How long a ref-store write may wait for the store lock, decided by what the
/// write is for rather than by how the result it saves turned out.
///
/// A command's reads run under its deadline, which `batch` narrows to what is
/// left of the batch budget. Saving a result that has already been observed is
/// a separate phase: the observation is paid for, so the save gets its own
/// bounded budget instead of whatever the observation left over. A complete
/// result and a partial one follow the same rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PersistBudget {
    /// The lock budget is capped by the inherited command deadline.
    Inherited,
    /// The lock budget starts when the save starts and ignores the inherited
    /// command deadline. It still ends after its own bound, so a save overruns
    /// the command deadline by at most that bound plus the write itself.
    DetachedAfterObservation,
}

impl PersistBudget {
    pub(crate) fn lock_deadline(self, budget_ms: u64) -> Result<Deadline, AppError> {
        let deadline = match self {
            Self::Inherited => Deadline::after(budget_ms)?,
            Self::DetachedAfterObservation => Deadline::detached_after(budget_ms)?,
        };
        Ok(deadline)
    }
}
