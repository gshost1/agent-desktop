/// What an aborted X11 sequence had to post to leave no key, button or keymap edit behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EmergencyRelease {
    /// Nothing was held or remapped when the sequence stopped, so no corrective input was posted.
    NotNeeded,
    /// Corrective input was posted; `acknowledged` is true only when the closing round trip came back.
    Posted { acknowledged: bool },
}
