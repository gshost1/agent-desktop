//! Which wait entries take a read before the entries that precede them, and
//! what they watch for.

use crate::cli::Commands;

/// The read a seeded wait entry diffs against, with the wait's whole filter:
/// an event baseline for a `wait` with `event`, or a notification listing for
/// a `wait` with `notification`. Two waits with equal kinds match exactly the
/// same changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SeedKind {
    Notification {
        app: Option<String>,
        text: Option<String>,
    },
    Event {
        event: String,
        app: Option<String>,
        window: Option<String>,
        window_id: Option<String>,
    },
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
                text: args.mode.text.clone(),
            });
        }
        args.event.event.clone().map(|event| Self::Event {
            event,
            app: args.app.clone(),
            window: args.mode.window.clone(),
            window_id: args.event.window_id.clone(),
        })
    }
}
