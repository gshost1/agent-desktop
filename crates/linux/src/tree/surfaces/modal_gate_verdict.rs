use agent_desktop_core::Deadline;

use crate::tree::AtspiElement;

pub(crate) struct ModalGateVerdict {
    pub(crate) deadline: Deadline,
    pub(crate) target: AtspiElement,
    pub(crate) frame: AtspiElement,
    pub(crate) windows: Vec<AtspiElement>,
}
