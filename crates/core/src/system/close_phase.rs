use crate::AppError;

/// The ordered steps of closing an application. Every step before `Closing`
/// sends nothing to the application, so a failure there is `not_delivered`
/// and safe to retry. `Closing` is the adapter's request and its own exit
/// wait; `ConfirmingExit` reads the process once more after a failed close,
/// so an application that is gone counts as closed instead of as an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClosePhase {
    Resolving,
    AwaitingLease,
    Revalidating,
    Closing,
    ConfirmingExit,
}

impl ClosePhase {
    /// The delivery a failure in this phase proves. Before the close request
    /// nothing reached the application; from `Closing` on, the adapter's own
    /// disposition stands.
    pub(crate) fn settle(self, error: impl Into<AppError>) -> AppError {
        let error = error.into();
        match self {
            Self::Resolving | Self::AwaitingLease | Self::Revalidating => {
                error.into_adapter_error().unsent().into()
            }
            Self::Closing | Self::ConfirmingExit => error,
        }
    }
}
