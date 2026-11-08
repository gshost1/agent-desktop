use crate::{
    AppError, NotificationIdentity, NotificationTarget, adapter::PlatformAdapter,
    context::CommandContext,
};
use serde_json::{Value, json};

pub struct DismissNotificationArgs {
    pub index: Option<usize>,
    pub key: Option<String>,
    pub app: Option<String>,
    pub expected_app: Option<String>,
    pub expected_title: Option<String>,
}

pub fn execute(
    args: DismissNotificationArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let identity = NotificationIdentity {
        expected_app: args.expected_app,
        expected_title: args.expected_title,
    };
    let target = NotificationTarget::new(args.index, args.key.as_deref(), identity)?;
    let policy = super::notification_policy::mutation_policy(context)?;
    let lease = crate::commands::helpers::acquire_interaction_lease(adapter)?;
    let dismissed = adapter.dismiss_notification(
        crate::DismissNotificationRequest {
            target: &target,
            app_filter: args.app.as_deref(),
            policy,
        },
        &lease,
    )?;
    Ok(json!({
        "dismissed": dismissed,
    }))
}
