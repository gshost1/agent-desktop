use std::time::Instant;

use serde_json::{Value, json};

use crate::{
    AppError, Deadline, NotificationBaseline, NotificationFilter, NotificationSeed,
    adapter::PlatformAdapter,
    commands::{
        wait::{WaitArgs, is_retryable_wait_poll_error},
        wait_mode::WaitMode,
        wait_timeout,
    },
    context::CommandContext,
};

pub(crate) fn wait_for_notification(
    app: Option<String>,
    text: Option<String>,
    timeout_ms: u64,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let filter = NotificationFilter {
        app,
        text,
        ..Default::default()
    };
    let seed = context.notification_seed();
    let baseline = seed.and_then(NotificationSeed::baseline).cloned();
    let result = poll(&filter, timeout_ms, baseline, adapter, context);
    with_fallback(result, seed.and_then(NotificationSeed::fallback))
}

fn poll(
    filter: &NotificationFilter,
    timeout_ms: u64,
    mut baseline: Option<NotificationBaseline>,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    if baseline.is_none() {
        adapter.check_notification_support(Deadline::standard()?)?;
    }
    let app = filter.app.clone();
    let text = filter.text.clone();
    let start = Instant::now();
    let deadline = crate::Deadline::at(start, timeout_ms)?;
    let mut scheduler = super::wait_scheduler::WaitScheduler::new(
        adapter,
        &crate::SignalFilter {
            app: app.clone(),
            process: None,
        },
        deadline,
    );
    let mut last_error = None;

    loop {
        if deadline.is_expired() {
            return wait_timeout::notification(app.as_ref(), text.as_ref(), timeout_ms, last_error);
        }
        match super::notification_policy::list_with_foreground_lease(
            filter, deadline, adapter, context,
        ) {
            Ok(current) => match baseline.as_mut() {
                None => {
                    baseline = Some(NotificationBaseline::from_list(&current));
                }
                Some(baseline) => {
                    if let Some(notif) = baseline.observe(&current) {
                        let elapsed = start.elapsed().as_millis();
                        return Ok(json!({
                            "condition": "notification",
                            "matched": true,
                            "notification": notif,
                            "elapsed_ms": elapsed,
                        }));
                    }
                }
            },
            Err(AppError::Adapter(err)) if wait_timeout::is_deadline_expired(&err) => {
                return wait_timeout::notification(
                    app.as_ref(),
                    text.as_ref(),
                    timeout_ms,
                    last_error,
                );
            }
            Err(AppError::Adapter(err)) if is_retryable_wait_poll_error(&err.code) => {
                last_error = Some(json!({
                    "code": err.code.as_str(),
                    "message": err.message
                }));
            }
            Err(err) => return Err(err),
        }

        let remaining = deadline.remaining();
        if remaining.is_zero() {
            return wait_timeout::notification(app.as_ref(), text.as_ref(), timeout_ms, last_error);
        }
        scheduler.wait(deadline);
    }
}

/// Reads what a batch hands the following `wait --notification` before it
/// runs the entry in between. Returns `None` unless `args` validates as a
/// notification wait. A headed wait is not seeded, so no notification surface
/// opens right before the action, and a failed read never blocks the action:
/// both fall back to the wait's own first list.
pub fn pre_action_seed(
    args: &WaitArgs,
    deadline: Deadline,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Option<NotificationSeed> {
    let Ok(WaitMode::Notification { app, text }) = WaitMode::from_args(args.clone()) else {
        return None;
    };
    if context.physical_input_policy().allow_focus_steal {
        return Some(NotificationSeed::SkippedHeaded);
    }
    let filter = NotificationFilter {
        app,
        text,
        ..Default::default()
    };
    let captured = adapter
        .check_notification_support(deadline)
        .map_err(AppError::from)
        .and_then(|()| {
            super::notification_policy::list_with_foreground_lease(
                &filter, deadline, adapter, context,
            )
        });
    Some(match captured {
        Ok(current) => NotificationSeed::Captured(NotificationBaseline::from_list(&current)),
        Err(_) => NotificationSeed::Unavailable,
    })
}

fn with_fallback(
    result: Result<Value, AppError>,
    fallback: Option<&'static str>,
) -> Result<Value, AppError> {
    let Some(fallback) = fallback else {
        return result;
    };
    match result {
        Ok(mut value) => {
            value["pre_action_seed"] = json!(fallback);
            Ok(value)
        }
        Err(AppError::Adapter(mut error)) => {
            let mut details = error.details.take().unwrap_or_else(|| json!({}));
            if let Value::Object(map) = &mut details {
                map.insert("pre_action_seed".into(), json!(fallback));
            }
            error.details = Some(details);
            Err(error.into())
        }
        Err(other) => Err(other),
    }
}

#[cfg(test)]
#[path = "wait_notification_tests.rs"]
mod tests;
