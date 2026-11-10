//! The read a seeded wait entry diffs against, held between the action it
//! was read before and the wait it belongs to.

use agent_desktop_core::{
    AdapterError, Deadline, NotificationSeed, PlatformAdapter, SignalBaseline, SignalFilter,
    commands::wait_notification as notification_wait, context::CommandContext,
};

use super::{preparation::PreparedCommand, seed_kind::SeedKind};

/// What a batch read hands its wait: an event baseline or a notification
/// listing.
pub(super) type SeedValues = (Option<SignalBaseline>, Option<NotificationSeed>);

/// What an entry's context is seeded with: an event baseline, possibly a
/// failed one handed down by the batch's own context, and a notification
/// listing.
pub(super) type EntrySeed = (
    Option<Result<SignalBaseline, AdapterError>>,
    Option<NotificationSeed>,
);

/// A read taken for one wait before the action at `read_at`. A failed read
/// is held too, so a wait that does not directly follow the action reports
/// it when reached instead of blocking the entries before it.
pub(super) struct PendingSeed {
    read_at: usize,
    read: Result<SeedValues, AdapterError>,
}

impl PendingSeed {
    /// Holds `read`, or nothing when the wait takes no read.
    pub(super) fn held(
        read_at: usize,
        read: Result<Option<SeedValues>, AdapterError>,
    ) -> Option<Self> {
        read.transpose().map(|read| Self { read_at, read })
    }

    /// The entry the read was taken before, and the read.
    pub(super) fn into_read(self) -> (usize, Result<SeedValues, AdapterError>) {
        (self.read_at, self.read)
    }
}

/// Reads what the wait entry `wait` diffs against, so a window, surface or
/// notification the action produces is reported as new instead of being
/// absorbed into the wait's own first read. Only an event baseline failure is
/// an error: a notification seed that cannot be read falls back to the
/// wait's own first list and never fails.
pub(super) fn read_seed(
    wait: &PreparedCommand,
    adapter: &dyn PlatformAdapter,
    deadline: Deadline,
) -> Result<Option<SeedValues>, AdapterError> {
    let Some((kind, args)) = SeedKind::of(&wait.command) else {
        return Ok(None);
    };
    match kind {
        SeedKind::Notification => {
            let Ok(core_args) = crate::dispatch::system::core_wait_args(args) else {
                return Ok(None);
            };
            let context = wait.context.clone().with_inherited_deadline(deadline);
            let seed = notification_wait::pre_action_seed(&core_args, deadline, adapter, &context);
            Ok(Some((None, seed)))
        }
        SeedKind::Event => {
            let filter = SignalFilter {
                app: args.app.clone(),
                process: None,
            };
            let baseline = adapter.capture_signal_baseline(&filter, deadline)?;
            Ok(Some((Some(baseline), None)))
        }
    }
}

/// Gives `context` the seed in `seed`, or none.
pub(super) fn seed_context(seed: Option<EntrySeed>, context: CommandContext) -> CommandContext {
    let (event, notification) = seed.unwrap_or_default();
    context
        .with_event_baseline(event)
        .with_notification_seed(notification)
}
