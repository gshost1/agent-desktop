//! Why a tool call gets no child process from the inherited-lease layer.

use agent_desktop_core::AppError;

/// `Cancelled` while the call waited for its turn, so no response is due. `Unusable` when the inherited
/// lease variable names nothing this server can lend, so the call fails before it starts.
pub(crate) enum LeaseRefusal {
    Cancelled,
    Unusable(AppError),
}
