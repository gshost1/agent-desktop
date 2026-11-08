//! Which wait entries take a read before the entries that precede them.

use crate::cli::Commands;

/// The read a seeded wait entry diffs against: an event baseline for a
/// `wait` with `event`, or a notification listing for a `wait` with
/// `notification`. Two waits with equal kinds watch for the same thing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SeedKind {
    Notification { app: Option<String> },
    Event { event: String, app: Option<String> },
}

impl SeedKind {
    /// The kind of read `command` takes, or none when it is not a seeded wait.
    pub(super) fn of(command: &Commands) -> Option<Self> {
        let Commands::Wait(args) = command else {
            return None;
        };
        if args.mode.notification {
            return Some(Self::Notification {
                app: args.app.clone(),
            });
        }
        args.event.event.clone().map(|event| Self::Event {
            event,
            app: args.app.clone(),
        })
    }
}
