use std::time::Duration;

use agent_desktop_core::Deadline;

use super::baseline::Baseline;
use crate::system::process::executable_identity::ExecutableIdentity;

/// How long a launch waits for a running spawned process to join the
/// accessibility bus before it reports the process without a window.
const NOT_ON_BUS_GRACE: Duration = Duration::from_millis(5_000);

/// The same wait for a launch asked to bring its window forward.
const NOT_ON_BUS_ACTIVATE_GRACE: Duration = Duration::from_millis(10_000);

/// What an owner search looks for and how long it may look. `not_on_bus`
/// ends the search early when the spawned process is still running, the
/// registry was read, and nothing on it is the launch's.
pub(super) struct OwnerQuery<'a> {
    pub(super) identity: &'a Option<ExecutableIdentity>,
    pub(super) before: &'a Baseline,
    pub(super) deadline: Deadline,
    pub(super) not_on_bus: Deadline,
}

impl<'a> OwnerQuery<'a> {
    pub(super) fn new(
        identity: &'a Option<ExecutableIdentity>,
        before: &'a Baseline,
        deadline: Deadline,
        activate: bool,
    ) -> Self {
        let grace = if activate {
            NOT_ON_BUS_ACTIVATE_GRACE
        } else {
            NOT_ON_BUS_GRACE
        };
        Self {
            identity,
            before,
            deadline,
            not_on_bus: deadline.capped(grace),
        }
    }
}
