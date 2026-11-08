use crate::{AppError, Deadline};

/// How long a ref-store write may wait for the store lock, decided by what the
/// write is for rather than by how the result it saves turned out.
///
/// A command's reads run under its deadline, which `batch` narrows to what is
/// left of the batch budget. Saving a result that has already been observed is
/// a separate phase: the observation is paid for, so the save gets its own
/// bounded budget instead of whatever the observation left over. A complete
/// result and a partial one follow the same rule. The trade-off: a save waits
/// up to its own bound for a contended lock even when the command still has
/// time left, and can finish after the command deadline by that bound plus
/// the write and prune pass (see `RefStore::save_new_snapshot`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PersistBudget {
    /// The lock budget is capped by the inherited command deadline. For lock
    /// work that does not save an observed result: garbage collection, the
    /// retained-token scan, the latest-pointer write and legacy migration.
    Inherited,
    /// The lock budget starts when the save starts and ignores the inherited
    /// command deadline. Every save of an observed refmap uses it.
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
