//! Why a tool call gets no child process from the inherited-lease layer.

use agent_desktop_core::AppError;

/// `Cancelled` while the call waited for its turn, so no response is due. `Refused` when the call fails
/// before it starts, with the error to answer: the inherited lease variable names nothing this server can
/// lend, an earlier call held the lease past the queue bound, or the call has no place in line.
pub(crate) enum LeaseRefusal {
    Cancelled,
    Refused(AppError),
}
