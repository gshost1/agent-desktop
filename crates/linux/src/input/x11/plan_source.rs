use agent_desktop_core::AdapterError;

use crate::input::keyboard::keymap::Keymap;
use crate::input::plan::Plan;

/// Plans built from the keymap the input worker reads on its own connection.
pub(crate) type KeyPlanner = Box<dyn FnOnce(&Keymap) -> Result<Vec<Plan>, AdapterError> + Send>;

/// The plans an input worker runs: built before it started, or built by the
/// worker from the keymap it reads right before sending, so a keyboard action
/// opens one X connection and sweeps the held-input journal once.
pub(crate) enum PlanSource {
    Ready(Vec<Plan>),
    FromKeymap(KeyPlanner),
}

impl From<Vec<Plan>> for PlanSource {
    fn from(plans: Vec<Plan>) -> Self {
        Self::Ready(plans)
    }
}
