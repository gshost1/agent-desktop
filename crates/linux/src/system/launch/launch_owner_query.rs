use std::cell::Cell;
use std::time::Duration;

use agent_desktop_core::Deadline;

use self::seen::Seen;
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

/// How long after a launcher exited cleanly the launch waits for a resident
/// to gain a window before it may take the one running instance as the owner.
const HANDOFF_SETTLE: Duration = Duration::from_millis(2_000);

#[path = "launch_off_bus.rs"]
pub(super) mod off_bus;
#[path = "launch_seen.rs"]
pub(super) mod seen;

/// What an owner search looks for and how long it may look. `not_on_bus`
/// ends the search early when the spawned process is still running, a
/// registry read that began after it expired answered, nothing on the
/// registry is the launch's, and the process will not join. `seen` keeps what
/// the search learned beyond the registry. `desktop_id` names the desktop
/// entry the launch started, and `settle` is how long a cleanly exited
/// launcher's handoff may take before the running instance is considered.
pub(super) struct OwnerQuery<'a> {
    pub(super) identity: &'a Option<ExecutableIdentity>,
    pub(super) before: &'a Baseline,
    pub(super) deadline: Deadline,
    pub(super) not_on_bus: Deadline,
    pub(super) seen: Cell<Seen>,
    pub(super) desktop_id: Option<&'a str>,
    pub(super) settle: Duration,
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
            seen: Cell::new(Seen::Nothing),
            desktop_id: None,
            settle: HANDOFF_SETTLE,
        }
    }

    /// The query for a launch of the desktop entry `desktop_id`.
    pub(super) fn for_entry(self, desktop_id: Option<&'a str>) -> Self {
        Self { desktop_id, ..self }
    }

    /// The query with a handoff settle of `settle` instead of the standard one.
    #[cfg(test)]
    pub(super) fn settling_after(self, settle: Duration) -> Self {
        Self { settle, ..self }
    }
}
