use std::cell::Cell;
use std::time::Duration;

use agent_desktop_core::Deadline;

use self::off_bus::OffBus;
use super::baseline::Baseline;
use crate::system::process::executable_identity::ExecutableIdentity;

/// How long a launch that was not asked to bring its window forward waits
/// for a running spawned process to join the accessibility bus before it may
/// report the process without a window. After it, the launch ends early only
/// on evidence that the process will not join ([`OffBus::never_joins`]); a
/// process that may still be starting is waited for until the deadline. A
/// launch asked to bring the window forward waits until its deadline, as its
/// window wait does.
const NOT_ON_BUS_GRACE: Duration = Duration::from_millis(5_000);

#[path = "launch_off_bus.rs"]
pub(super) mod off_bus;

/// What an owner search looks for and how long it may look. `not_on_bus`
/// ends the search early when the spawned process is still running, a
/// registry read that began after it expired answered, nothing on the
/// registry is the launch's, and the process will not join. `off_bus` keeps
/// what the search last saw of the process off the bus.
pub(super) struct OwnerQuery<'a> {
    pub(super) identity: &'a Option<ExecutableIdentity>,
    pub(super) before: &'a Baseline,
    pub(super) deadline: Deadline,
    pub(super) not_on_bus: Deadline,
    pub(super) off_bus: Cell<OffBus>,
}

impl<'a> OwnerQuery<'a> {
    pub(super) fn new(
        identity: &'a Option<ExecutableIdentity>,
        before: &'a Baseline,
        deadline: Deadline,
        activate: bool,
    ) -> Self {
        let grace = if activate {
            None
        } else {
            Some(NOT_ON_BUS_GRACE)
        };
        Self::with_grace(identity, before, deadline, grace)
    }

    /// A query whose not-on-bus wait is `grace`, or the whole deadline for
    /// `None`.
    pub(super) fn with_grace(
        identity: &'a Option<ExecutableIdentity>,
        before: &'a Baseline,
        deadline: Deadline,
        grace: Option<Duration>,
    ) -> Self {
        Self {
            identity,
            before,
            deadline,
            not_on_bus: grace.map_or(deadline, |grace| deadline.capped(grace)),
            off_bus: Cell::new(OffBus::Starting),
        }
    }
}
