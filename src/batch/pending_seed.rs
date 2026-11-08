//! The pre-action read a seeded wait entry diffs against, held between the
//! entry it was read before and the wait it belongs to.

use agent_desktop_core::{
    AdapterError, Deadline, NotificationSeed, PlatformAdapter, SignalBaseline, SignalFilter,
    commands::wait_notification as notification_wait, context::CommandContext,
};

use crate::cli::Commands;

use super::preparation::PreparedCommand;

/// A read taken for one wait entry, handed only to the entry at `for_index`,
/// so a wait never runs against a read taken for another entry.
pub(super) struct PendingSeed {
    for_index: usize,
    event: Option<SignalBaseline>,
    notification: Option<NotificationSeed>,
}

impl PendingSeed {
    /// Reads what the wait entry at `wait_index` diffs against, so a window,
    /// surface or notification the entries before it produce is reported as
    /// new instead of being absorbed into the wait's own first read. Only an
    /// event baseline failure is an error: a notification seed that cannot be
    /// read falls back to the wait's own first list and never blocks.
    pub(super) fn capture(
        wait_index: usize,
        wait: &PreparedCommand,
        adapter: &dyn PlatformAdapter,
        deadline: Deadline,
    ) -> Result<Option<Self>, AdapterError> {
        let Commands::Wait(args) = &wait.command else {
            return Ok(None);
        };
        let (event, notification) = if args.mode.notification {
            let Ok(core_args) = crate::dispatch::system::core_wait_args(args) else {
                return Ok(None);
            };
            let context = wait.context.clone().with_inherited_deadline(deadline);
            let seed = notification_wait::pre_action_seed(&core_args, deadline, adapter, &context);
            (None, seed)
        } else if args.event.event.is_some() {
            let filter = SignalFilter {
                app: args.app.clone(),
                process: None,
            };
            (
                Some(adapter.capture_signal_baseline(&filter, deadline)?),
                None,
            )
        } else {
            return Ok(None);
        };
        Ok(Some(Self {
            for_index: wait_index,
            event,
            notification,
        }))
    }

    /// Takes the read out of `slot` when it belongs to the entry at `index`.
    pub(super) fn take_for(slot: &mut Option<Self>, index: usize) -> Option<Self> {
        if slot.as_ref().is_some_and(|seed| seed.for_index == index) {
            slot.take()
        } else {
            None
        }
    }

    /// Gives `context` the read this seed holds, or none when `seed` is empty.
    pub(super) fn seed_context(seed: Option<Self>, context: CommandContext) -> CommandContext {
        let (event, notification) = seed
            .map(|seed| (seed.event, seed.notification))
            .unwrap_or_default();
        context
            .with_event_baseline(event.map(Ok))
            .with_notification_seed(notification)
    }
}
