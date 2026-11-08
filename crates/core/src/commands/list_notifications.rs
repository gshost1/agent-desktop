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
    let everything = super::notification_policy::list_with_foreground_lease(
        &NotificationFilter::default(),
        crate::Deadline::standard()?,
        adapter,
        context,
    )?;
    let notifications = keyed(&everything, &filter)?;
    Ok(json!({
        "count": notifications.len(),
        "notifications": notifications,
    }))
}

/// The entries of the whole list that pass `filter`, each with the `key` a
/// mutation names it by. Keys are assigned over the whole list first, so no
/// filter or limit can change one.
fn keyed(
    everything: &[NotificationInfo],
    filter: &NotificationFilter,
) -> Result<Vec<Value>, AppError> {
    everything
        .iter()
        .zip(crate::notification_keys(everything))
        .filter(|(info, _)| filter.admits(info))
        .take(filter.limit.unwrap_or(usize::MAX))
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
