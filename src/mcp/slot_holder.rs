//! Who holds the inherited interaction lease, and how far a cancelled holder is from being reaped.

use std::thread::JoinHandle;

use super::lease_slot::Ticket;

/// `Lent` while a call's child runs; its call thread reaps it and frees the slot. `Cancelling` once that
/// call was cancelled and before its child, if any, was handed to a reaper. `Reaping` until that reaper,
/// when there is one, has joined: only then is the child known to have exited.
#[derive(Default)]
pub(crate) enum SlotHolder {
    #[default]
    Free,
    Lent(Ticket),
    Cancelling(Ticket),
    Reaping(Ticket, Option<JoinHandle<()>>),
}
