use std::sync::{Arc, Mutex, PoisonError};

use agent_desktop_core::Deadline;

use super::modal_gate_verdict::ModalGateVerdict;
use crate::tree::AtspiElement;

#[derive(Clone, Default)]
pub(crate) struct GateMemo(Arc<Mutex<Option<ModalGateVerdict>>>);

impl GateMemo {
    pub(crate) fn matches(
        &self,
        deadline: Deadline,
        key: (&AtspiElement, &AtspiElement),
        windows: &[AtspiElement],
    ) -> bool {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .is_some_and(|kept| {
                kept.deadline == deadline
                    && &kept.target == key.0
                    && &kept.frame == key.1
                    && kept.windows.len() == windows.len()
                    && windows.iter().all(|window| kept.windows.contains(window))
            })
    }

    pub(crate) fn store(&self, verdict: ModalGateVerdict) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = Some(verdict);
    }

    pub(crate) fn clear(&self) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }
}
