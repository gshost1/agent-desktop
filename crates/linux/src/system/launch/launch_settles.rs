use std::time::Duration;

/// How long evidence must hold before an owner search acts on it: a cleanly
/// exited launcher's handoff before the running instance may be taken as the
/// owner, and a window of a spawned process off the bus before the process
/// is taken never to join, since a toolkit may register a moment after it
/// maps its window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Settles {
    pub(crate) handoff: Duration,
    pub(crate) window_off_bus: Duration,
}
