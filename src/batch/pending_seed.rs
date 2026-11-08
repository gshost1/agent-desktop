//! The read a seeded wait entry diffs against, held between the entry it was
//! read at and the wait it belongs to.

use agent_desktop_core::{
    AdapterError, Deadline, NotificationSeed, PlatformAdapter, SignalBaseline, SignalFilter,
    commands::wait_notification as notification_wait, context::CommandContext,
};

use crate::cli::Commands;

use super::{preparation::PreparedCommand, seed_kind::SeedKind};

pub(super) type SeedValues = (Option<SignalBaseline>, Option<NotificationSeed>);

/// A read taken for one wait entry at an earlier entry, handed only to the
/// entry at `for_index`. A failed read is held too, so the wait reports it
/// when it is reached instead of blocking the entry the read was taken at.
pub(super) struct PendingSeed {
    for_index: usize,
    read_at: usize,
    read: Result<SeedValues, AdapterError>,
}

impl PendingSeed {
    /// Holds `read` for the wait at `for_index`, or nothing when the wait
    /// takes no read.
    pub(super) fn held(
        for_index: usize,
        read_at: usize,
        read: Result<Option<SeedValues>, AdapterError>,
    ) -> Option<Self> {
        read.transpose().map(|read| Self {
            for_index,
            read_at,
            read,
        })
    }

    pub(super) fn is_for(&self, index: usize) -> bool {
        self.for_index == index
    }

    /// The entry the read was taken at, and the read.
    pub(super) fn into_read(self) -> (usize, Result<SeedValues, AdapterError>) {
        (self.read_at, self.read)
    }
}

/// Reads what the wait entry `wait` diffs against, so a window, surface or
/// notification the entries before it produce is reported as new instead of
/// being absorbed into the wait's own first read. Only an event baseline
/// failure is an error: a notification seed that cannot be read falls back
/// to the wait's own first list and never fails.
pub(super) fn read_seed(
    wait: &PreparedCommand,
    adapter: &dyn PlatformAdapter,
    deadline: Deadline,
) -> Result<Option<SeedValues>, AdapterError> {
    let (Commands::Wait(args), Some(kind)) = (&wait.command, SeedKind::of(&wait.command)) else {
        return Ok(None);
    };
    match kind {
        SeedKind::Notification { .. } => {
            let Ok(core_args) = crate::dispatch::system::core_wait_args(args) else {
                return Ok(None);
            };
            let context = wait.context.clone().with_inherited_deadline(deadline);
            let seed = notification_wait::pre_action_seed(&core_args, deadline, adapter, &context);
            Ok(Some((None, seed)))
        }
        SeedKind::Event { app, .. } => {
            let filter = SignalFilter { app, process: None };
            let baseline = adapter.capture_signal_baseline(&filter, deadline)?;
            Ok(Some((Some(baseline), None)))
        }
    }
}

/// Gives `context` the read in `values`, or none.
pub(super) fn seed_context(values: Option<SeedValues>, context: CommandContext) -> CommandContext {
    let (event, notification) = values.unwrap_or_default();
    context
        .with_event_baseline(event.map(Ok))
        .with_notification_seed(notification)
}
