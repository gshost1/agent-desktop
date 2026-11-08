use crate::{
    AppError, NotificationFilter, NotificationInfo, adapter::PlatformAdapter,
    context::CommandContext,
};
use serde_json::{Value, json};

pub struct ListNotificationsArgs {
    pub app: Option<String>,
    pub text: Option<String>,
    pub limit: Option<usize>,
}

pub fn execute(
    args: ListNotificationsArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let filter = NotificationFilter {
        app: args.app,
        text: args.text,
        limit: args.limit,
    };
    let notifications = super::notification_policy::list_with_foreground_lease(
        &filter,
        crate::Deadline::standard()?,
        adapter,
        context,
    )?;
    Ok(json!({
        "count": notifications.len(),
        "notifications": keyed(&notifications)?,
    }))
}

/// Each entry with the `key` a mutation names it by.
fn keyed(notifications: &[NotificationInfo]) -> Result<Vec<Value>, AppError> {
    notifications
        .iter()
        .zip(crate::notification_keys(notifications))
        .map(|(info, key)| {
            let mut entry = serde_json::to_value(info)?;
            entry["key"] = json!(key);
            Ok(entry)
        })
        .collect()
}

#[cfg(test)]
#[path = "list_notifications_tests.rs"]
mod tests;
