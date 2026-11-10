use crate::{
    AppError, CommandContext, NotificationIdentity, NotificationTarget, adapter::PlatformAdapter,
};
use serde_json::Value;

pub struct NotificationActionArgs {
    pub index: usize,
    pub key: Option<String>,
    pub action: String,
    pub expected_app: Option<String>,
    pub expected_title: Option<String>,
}

pub fn execute(
    args: NotificationActionArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let identity = NotificationIdentity {
        expected_app: args.expected_app,
        expected_title: args.expected_title,
    };
    let target = NotificationTarget::new(Some(args.index), args.key.as_deref(), identity)?;
    let policy = super::notification_policy::mutation_policy(context)?;
    let lease = crate::commands::helpers::acquire_interaction_lease(adapter)?;
    let result = adapter.notification_action(
        crate::NotificationActionRequest {
            target: &target,
            action_name: &args.action,
            policy,
        },
        &lease,
    )?;
    Ok(serde_json::to_value(result)?)
}
