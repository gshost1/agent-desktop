use crate::{InteractionPolicy, NotificationTarget};

pub struct NotificationActionRequest<'a> {
    pub target: &'a NotificationTarget,
    pub action_name: &'a str,
    pub policy: InteractionPolicy,
}
