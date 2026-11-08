//! What one dispatch attempt of a ref action reports to the auto-wait loop.

use crate::{ActionResult, AdapterError};

/// The result of one dispatch attempt, and whether a focus call ran before
/// it. The loop keeps the second across attempts: once a command has raised
/// the target, it does not go back to the wait for another attempt, so a
/// command raises a window at most once.
pub(crate) struct SingleShot {
    pub(crate) result: Result<ActionResult, AdapterError>,
    pub(crate) focused: bool,
}
