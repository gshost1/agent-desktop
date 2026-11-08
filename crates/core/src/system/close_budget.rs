use std::time::Duration;

use serde_json::{Map, Value};

use crate::{AdapterError, AppError, Deadline, ErrorCode, system::close_phase::ClosePhase};

const MIN_CONFIRM_SLICE: Duration = Duration::from_millis(50);
const MAX_CONFIRM_SLICE: Duration = Duration::from_millis(250);

/// One close deadline split into the slices its phases may spend. Resolving
/// the app and waiting for the interaction lease share the first half; the
/// second half is reserved for revalidating, sending the close and waiting
/// for the exit, so a contended lease cannot leave the exit wait with
/// nothing. The last part of that reserve, a quarter of it kept between
/// `MIN_CONFIRM_SLICE` and `MAX_CONFIRM_SLICE` and never more than half of
/// it, is kept for watching the process after a failed close.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CloseBudget {
    pre_close: Deadline,
    closing: Deadline,
    confirming: Deadline,
    pre_close_slice: Duration,
    closing_slice: Duration,
    confirm_slice: Duration,
}

impl CloseBudget {
    pub(crate) fn split(total: Deadline) -> Self {
        let remaining = total.remaining();
        let exit_reserve = remaining / 2;
        let confirm_slice = (exit_reserve / 4)
            .clamp(MIN_CONFIRM_SLICE, MAX_CONFIRM_SLICE)
            .min(exit_reserve / 2);
        let pre_close_slice = remaining.saturating_sub(exit_reserve);
        let closing_slice = remaining.saturating_sub(confirm_slice);
        Self {
            pre_close: total.capped(pre_close_slice),
            closing: total.capped(closing_slice),
            confirming: total,
            pre_close_slice,
            closing_slice,
            confirm_slice,
        }
    }

    pub(crate) fn deadline(self, phase: ClosePhase) -> Deadline {
        match phase {
            ClosePhase::Resolving | ClosePhase::AwaitingLease => self.pre_close,
            ClosePhase::Revalidating | ClosePhase::Closing => self.closing,
            ClosePhase::ConfirmingExit => self.confirming,
        }
    }

    fn slice(self, phase: ClosePhase) -> Duration {
        match phase {
            ClosePhase::Resolving | ClosePhase::AwaitingLease => self.pre_close_slice,
            ClosePhase::Revalidating | ClosePhase::Closing => self.closing_slice,
            ClosePhase::ConfirmingExit => self.confirm_slice,
        }
    }

    /// The failure as the caller sees it. A phase that sends nothing settles
    /// as `not_delivered`, and a `TIMEOUT` names the phase and the slice that
    /// ran out rather than the whole close budget.
    pub(crate) fn fail(self, phase: ClosePhase, error: impl Into<AppError>) -> AppError {
        let mut error = error.into().into_adapter_error();
        if phase.sends_nothing() {
            error = error.unsent();
        }
        if error.code == ErrorCode::Timeout {
            error = with_phase_details(error, phase, self.slice(phase));
        }
        error.into()
    }
}

fn with_phase_details(error: AdapterError, phase: ClosePhase, slice: Duration) -> AdapterError {
    let mut details = match error.details.clone() {
        Some(Value::Object(map)) => map,
        _ => Map::new(),
    };
    details.insert("phase".into(), Value::from(phase.label()));
    details.insert(
        "timeout_ms".into(),
        Value::from(u64::try_from(slice.as_millis()).unwrap_or(u64::MAX)),
    );
    error.with_details(Value::Object(details))
}

#[cfg(test)]
#[path = "close_budget_tests.rs"]
mod tests;
