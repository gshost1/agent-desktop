/// The ordered steps of closing an application. Every step before `Closing`
/// sends nothing to the application, so a failure there is `not_delivered`
/// and safe to retry. `Closing` is the adapter's request and its own exit
/// wait; `ConfirmingExit` watches the process after a failed close, so an
/// application that exits counts as closed instead of as an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClosePhase {
    Resolving,
    AwaitingLease,
    Revalidating,
    Closing,
    ConfirmingExit,
}

impl ClosePhase {
    pub(crate) fn sends_nothing(self) -> bool {
        matches!(
            self,
            Self::Resolving | Self::AwaitingLease | Self::Revalidating
        )
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Resolving => "resolving",
            Self::AwaitingLease => "awaiting_lease",
            Self::Revalidating => "revalidating",
            Self::Closing => "closing",
            Self::ConfirmingExit => "confirming_exit",
        }
    }
}
