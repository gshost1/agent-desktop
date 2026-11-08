use crate::{
    AdapterError, CursorOverlayConfig, InteractionPolicy, NotificationBaseline, SignalBaseline,
};

use super::WaitSelector;

#[derive(Debug, Clone, Default)]
pub(super) struct CommandOptions {
    pub interaction_policy: InteractionPolicy,
    pub wait_selector: Option<WaitSelector>,
    pub event_baseline: Option<Result<SignalBaseline, AdapterError>>,
    pub notification_baseline: Option<NotificationBaseline>,
    pub cursor_overlay: CursorOverlayConfig,
    pub agent_id: Option<String>,
}

impl CommandOptions {
    pub fn for_batch(&self, cursor_overlay: CursorOverlayConfig) -> Self {
        Self {
            interaction_policy: self.interaction_policy,
            wait_selector: None,
            event_baseline: None,
            notification_baseline: None,
            cursor_overlay,
            agent_id: self.agent_id.clone(),
        }
    }
}
