use crate::{InteractionPolicy, NotificationTarget};

pub struct DismissNotificationRequest<'a> {
    pub target: &'a NotificationTarget,
    pub policy: InteractionPolicy,
}
