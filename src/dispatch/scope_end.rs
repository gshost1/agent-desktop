//! Closes a command's trace scope. Under `--trace-strict` a failed
//! `command.end` fails the command. A batch has already run its entries by
//! then, so its body, with every per-entry result and disposition, travels in
//! the error's `details.batch` instead of being dropped.

use agent_desktop_core::{AppError, DeliveryDisposition, DeliverySemantics, context::CommandScope};
use serde_json::{Value, json};

pub(super) fn complete(
    scope: CommandScope<'_>,
    result: Result<Value, AppError>,
    batch: bool,
) -> Result<Value, AppError> {
    match (scope.complete(&result), result) {
        (Ok(()), result) => result,
        (Err(trace_error), Ok(body)) if batch => Err(batch_trace_failure(trace_error, body)),
        (Err(trace_error), _) => Err(trace_error),
    }
}

fn batch_trace_failure(trace_error: AppError, body: Value) -> AppError {
    let disposition = batch_disposition(&body);
    let mut error = trace_error.into_adapter_error();
    let mut details = json!({ "kind": "batch_trace_end_failed", "batch": body });
    if let Some(cause_details) = error.details.take() {
        details["cause_details"] = cause_details;
    }
    error
        .with_details(details)
        .with_disposition(disposition)
        .into()
}

fn batch_disposition(body: &Value) -> DeliverySemantics {
    let sent_nothing = body
        .get("results")
        .and_then(Value::as_array)
        .is_some_and(|entries| entries.iter().all(entry_sent_nothing));
    if sent_nothing {
        DeliverySemantics::not_delivered()
    } else {
        DeliverySemantics::uncertain()
    }
}

fn entry_sent_nothing(entry: &Value) -> bool {
    entry
        .get("data")
        .and_then(|data| data.get("disposition"))
        .or_else(|| {
            entry
                .get("error")
                .and_then(|error| error.get("disposition"))
        })
        .cloned()
        .and_then(|value| serde_json::from_value::<DeliverySemantics>(value).ok())
        .is_some_and(|disposition| disposition.delivery() == DeliveryDisposition::NotDelivered)
}

#[cfg(test)]
#[path = "scope_end_tests.rs"]
mod tests;
