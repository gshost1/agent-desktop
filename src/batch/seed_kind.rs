//! Which wait entries take a read before the action they observe.

use crate::{cli::Commands, cli_args::system::WaitArgs};

/// The read a seeded wait entry diffs against: an event baseline for a
/// `wait` with `event`, a notification listing for a `wait` with
/// `notification`, or one menu reading for a `wait` with `menu_closed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SeedKind {
    Notification,
    Event,
    MenuClosed,
}

impl SeedKind {
    /// The kind of read `command` takes with the wait's arguments, or none
    /// when it is not a seeded wait.
    pub(super) fn of(command: &Commands) -> Option<(Self, &WaitArgs)> {
        let Commands::Wait(args) = command else {
            return None;
        };
        if args.surface.menu_closed {
            Some((Self::MenuClosed, args))
        } else if args.surface.notification {
            Some((Self::Notification, args))
        } else if args.event.event.is_some() {
            Some((Self::Event, args))
        } else {
            None
        }
    }
}
