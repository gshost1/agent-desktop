//! What a call does next with the inherited-lease slot.

use std::thread::JoinHandle;

/// While queued: `Wait`, `Take` the free slot when first in line, or `Leave` once cancelled. While ending
/// its turn: `Wait` for the cancelled child's reaper, `Join` that reaper, or find the slot `Freed`.
pub(crate) enum SlotStep {
    Wait,
    Take,
    Leave,
    Join(JoinHandle<()>),
    Freed,
}
