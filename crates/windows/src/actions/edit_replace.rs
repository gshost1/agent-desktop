//! Maps the answers to the two messages that replace an edit's text onto the
//! delivery a value write reports.
//!
//! `EM_SETSEL` selects the whole text and changes nothing the owner reads, so
//! a selection that fails or goes unanswered reports the write not delivered
//! and no text is sent. `EM_REPLACESEL` is the write itself: once it is
//! queued for the edit's thread it may still land after the wait ends, so an
//! unanswered replace reports delivered and unverified. A replace that was
//! never handed to the edit's thread is not delivered.

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};

/// One message a text replacement sends to an edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EditMessage {
    /// `EM_SETSEL(0, -1)`, which selects the whole text.
    SelectAll,
    /// `EM_REPLACESEL(TRUE, text)` with a NUL-terminated UTF-16 buffer.
    ReplaceSelection(Vec<u16>),
}

/// Why a message to an edit did not complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EditSendFailure {
    /// The message never reached the edit's thread.
    NotSent(String),
    /// The message reached the edit's thread and was not answered in time.
    Unanswered(String),
}

/// Sends a message to an edit window and waits for its answer.
pub(crate) trait EditMessenger {
    fn send(
        &self,
        handle: isize,
        message: EditMessage,
        deadline: Deadline,
    ) -> Result<(), EditSendFailure>;
}

/// Replaces the edit's whole text with `value`: selects all of it, then
/// replaces the selection.
pub(crate) fn replace_text(
    messenger: &impl EditMessenger,
    handle: isize,
    value: &str,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    messenger
        .send(handle, EditMessage::SelectAll, deadline)
        .map_err(|failure| failed("EM_SETSEL", failure, DeliverySemantics::not_delivered()))?;
    let text = value.encode_utf16().chain(std::iter::once(0)).collect();
    match messenger.send(handle, EditMessage::ReplaceSelection(text), deadline) {
        Ok(()) => Ok(()),
        Err(failure @ EditSendFailure::NotSent(_)) => Err(failed(
            "EM_REPLACESEL",
            failure,
            DeliverySemantics::not_delivered(),
        )),
        Err(failure) => Err(failed(
            "EM_REPLACESEL",
            failure,
            DeliverySemantics::delivered_unverified(),
        )),
    }
}

fn failed(message: &str, failure: EditSendFailure, disposition: DeliverySemantics) -> AdapterError {
    let (summary, detail) = match failure {
        EditSendFailure::NotSent(detail) => (
            "The text replacement was not sent to the edit control",
            detail,
        ),
        EditSendFailure::Unanswered(detail) => (
            "The edit control did not answer the text replacement in time",
            detail,
        ),
    };
    AdapterError::new(ErrorCode::ActionFailed, summary)
        .with_platform_detail(format!("{message}: {detail}"))
        .with_disposition(disposition)
}

#[cfg(test)]
#[path = "edit_replace_tests.rs"]
mod tests;
