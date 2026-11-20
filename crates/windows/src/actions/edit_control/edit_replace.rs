//! Maps the steps that replace an edit's text onto the delivery a value
//! write reports.
//!
//! A write first reserves what its replace needs, then selects the whole
//! text with `EM_SETSEL`, then replaces the selection with `EM_REPLACESEL`.
//! A refused reservation sends nothing. `EM_SETSEL` changes nothing the
//! owner reads, so a selection that fails or goes unanswered reports the
//! write not delivered and no text is sent. `EM_REPLACESEL` is the write
//! itself: once it is queued for the edit's thread it may still land after
//! the wait ends, so an unanswered replace reports delivered and unverified.
//! A replace that never reached the edit's thread is not delivered.

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};

/// Why a step of a text replacement did not complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EditSendFailure {
    /// The step never reached the edit's thread.
    NotSent(String),
    /// The message reached the edit's thread and was not answered in time.
    Unanswered(String),
}

/// Sends the messages of a text replacement to an edit window.
pub(crate) trait EditMessenger {
    /// What a replace needs held from before the selection until it returns.
    type Reservation;

    /// Takes what the replace to `handle` needs, before anything is sent.
    fn reserve(&self, handle: isize) -> Result<Self::Reservation, EditSendFailure>;

    /// `EM_SETSEL(0, -1)`, which selects the whole text.
    fn select_all(&self, handle: isize, deadline: Deadline) -> Result<(), EditSendFailure>;

    /// `EM_REPLACESEL(TRUE, text)` with a NUL-terminated UTF-16 buffer, to
    /// the edit the reservation was taken for.
    fn replace_selection(
        &self,
        reservation: Self::Reservation,
        text: Vec<u16>,
        deadline: Deadline,
    ) -> Result<(), EditSendFailure>;
}

/// Replaces the edit's whole text with `value`: reserves the replace,
/// selects all of the text, then replaces the selection.
pub(crate) fn replace_text<M: EditMessenger>(
    messenger: &M,
    handle: isize,
    value: &str,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let reservation = messenger
        .reserve(handle)
        .map_err(|failure| failed("EM_REPLACESEL", failure, DeliverySemantics::not_delivered()))?;
    messenger
        .select_all(handle, deadline)
        .map_err(|failure| failed("EM_SETSEL", failure, DeliverySemantics::not_delivered()))?;
    let text = value.encode_utf16().chain(std::iter::once(0)).collect();
    match messenger.replace_selection(reservation, text, deadline) {
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
