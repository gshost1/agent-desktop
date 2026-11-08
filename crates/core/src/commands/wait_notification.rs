use std::time::Instant;

use serde_json::{Value, json};

use crate::{
    AdapterError, AppError, Deadline, NotificationBaseline, NotificationFilter,
    adapter::PlatformAdapter,
    commands::{wait::is_retryable_wait_poll_error, wait_timeout},
    context::CommandContext,
};

pub(crate) fn wait_for_notification(
    app: Option<String>,
    text: Option<String>,
    timeout_ms: u64,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    adapter.check_notification_support(crate::Deadline::standard()?)?;
    let filter = NotificationFilter {
        app: app.clone(),
        text: text.clone(),
        ..Default::default()
    };
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
    let mut baseline: Option<NotificationBaseline> = context.notification_baseline().cloned();
    let mut last_error = None;

    loop {
        if deadline.is_expired() {
            return wait_timeout::notification(app.as_ref(), text.as_ref(), timeout_ms, last_error);
        }
        match super::notification_policy::list_with_foreground_lease(
            &filter, deadline, adapter, context,
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

/// Lists the notifications `filter` selects and records them, so that a
/// `wait --notification` seeded with the result reports a notification that
/// arrives between this call and the wait's first list.
pub fn capture_notification_baseline(
    filter: &NotificationFilter,
    deadline: Deadline,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<NotificationBaseline, AdapterError> {
    adapter.check_notification_support(deadline)?;
    super::notification_policy::list_with_foreground_lease(filter, deadline, adapter, context)
        .map(|current| NotificationBaseline::from_list(&current))
        .map_err(AppError::into_adapter_error)
}

#[cfg(test)]
#[path = "wait_notification_tests.rs"]
mod tests;
