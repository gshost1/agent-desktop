//! Closes a command's trace scope. Under `--trace-strict` a failed
//! `command.end` fails the command. A batch has already run its entries by
//! then, so its body, with every per-entry result and disposition, travels in
//! the error's `details.batch` instead of being dropped.

use agent_desktop_core::{AppError, CommandScope, DeliveryDisposition, DeliverySemantics};
use serde_json::Value;

use crate::command_policy::CommandEffect;

/// `batch_effects` is the effect of each batch entry by index, present only
/// when the command is a batch.
pub(super) fn complete(
    scope: CommandScope<'_>,
    result: Result<Value, AppError>,
    batch_effects: Option<&[CommandEffect]>,
) -> Result<Value, AppError> {
    match (scope.complete(&result), result, batch_effects) {
        (Ok(()), result, _) => result,
        (Err(trace_error), Ok(body), Some(effects)) => {
            Err(batch_trace_failure(trace_error, body, effects))
        }
        (Err(trace_error), _, _) => Err(trace_error),
    }
}

fn batch_trace_failure(trace_error: AppError, body: Value, effects: &[CommandEffect]) -> AppError {
    let disposition = batch_disposition(&body, effects);
    let mut error = trace_error.into_adapter_error();
    let cause_details = error.details.take();
    let mut details = crate::batch::trace_failure_details(body);
    if let Some(cause_details) = cause_details {
        details["cause_details"] = cause_details;
    }
    error
        .with_details(details)
        .with_disposition(disposition)
        .into()
}

fn batch_disposition(body: &Value, effects: &[CommandEffect]) -> DeliverySemantics {
    let sent_nothing = body
        .get("results")
        .and_then(Value::as_array)
        .is_some_and(|entries| {
            entries.iter().all(|entry| {
                let effect = entry
                    .get("index")
                    .and_then(Value::as_u64)
                    .and_then(|index| effects.get(usize::try_from(index).ok()?))
                    .copied()
                    .unwrap_or(CommandEffect::AppInput);
                crate::batch::entry_disposition(entry, effect).delivery()
                    == DeliveryDisposition::NotDelivered
            })
        });
    if sent_nothing {
        DeliverySemantics::not_delivered()
    } else {
        DeliverySemantics::uncertain()
    }
}

#[cfg(test)]
#[path = "scope_end_tests.rs"]
mod tests;
