//! Decides how a text replacement reaches an edit from where the edit's
//! window lives relative to the caller.
//!
//! A window of the calling thread runs its window procedure inside the send.
//! For a window of another process the system marshals the text into that
//! process, since it cannot read the sender's memory, so the replace keeps
//! the inline `SendMessageTimeoutW` and frees the buffer on return, as
//! before. Only a window of another thread in this process reads the
//! sender's own buffer, whenever its thread gets to the message, so only
//! that case keeps the buffer alive past a timed-out send.

/// The thread and process a window belongs to, or the caller runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ThreadIdentity {
    pub(crate) thread: u32,
    pub(crate) process: u32,
}

/// Where an edit's window lives relative to the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditHome {
    /// The calling thread owns the window.
    CallingThread,
    /// Another thread of the calling process owns the window.
    ThisProcess,
    /// Another process owns the window, or the window is gone.
    OtherProcess,
}

impl EditHome {
    pub(crate) fn of(window: ThreadIdentity, caller: ThreadIdentity) -> Self {
        if window.thread == 0 || window.process != caller.process {
            Self::OtherProcess
        } else if window.thread == caller.thread {
            Self::CallingThread
        } else {
            Self::ThisProcess
        }
    }

    /// Whether the replacement's text must outlive a send that timed out.
    pub(crate) fn needs_owned_text(self) -> bool {
        self == Self::ThisProcess
    }
}

#[cfg(test)]
#[path = "edit_route_tests.rs"]
mod tests;
