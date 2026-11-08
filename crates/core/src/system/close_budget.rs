use std::time::Duration;

use crate::{Deadline, system::close_phase::ClosePhase};

const MAX_CONFIRM_SLICE: Duration = Duration::from_millis(250);

/// One close deadline split into the slices its phases may spend. Resolving
/// the app and waiting for the interaction lease share the first half; the
/// second half is reserved for revalidating, sending the close and waiting
/// for the exit, so a contended lease cannot leave the exit wait with
/// nothing. The last slice of that reserve, at most `MAX_CONFIRM_SLICE`, is
/// kept for one liveness read after a failed close.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CloseBudget {
    pre_close: Deadline,
    closing: Deadline,
    confirming: Deadline,
}

impl CloseBudget {
    pub(crate) fn split(total: Deadline) -> Self {
        let remaining = total.remaining();
        let exit_reserve = remaining / 2;
        let confirm_slice = (exit_reserve / 4).min(MAX_CONFIRM_SLICE);
        Self {
            pre_close: total.capped(remaining.saturating_sub(exit_reserve)),
            closing: total.capped(remaining.saturating_sub(confirm_slice)),
            confirming: total,
        }
    }

    pub(crate) fn deadline(self, phase: ClosePhase) -> Deadline {
        match phase {
            ClosePhase::Resolving | ClosePhase::AwaitingLease => self.pre_close,
            ClosePhase::Revalidating | ClosePhase::Closing => self.closing,
            ClosePhase::ConfirmingExit => self.confirming,
        }
    }
}

#[cfg(test)]
#[path = "close_budget_tests.rs"]
mod tests;
