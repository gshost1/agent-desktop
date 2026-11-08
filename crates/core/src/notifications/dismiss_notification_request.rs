use crate::{InteractionPolicy, NotificationTarget};

pub struct DismissNotificationRequest<'a> {
    pub target: &'a NotificationTarget,
    pub app_filter: Option<&'a str>,
    pub policy: InteractionPolicy,
}
