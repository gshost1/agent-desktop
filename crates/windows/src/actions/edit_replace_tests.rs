use super::{EditMessage, EditMessenger, EditSendFailure, replace_text};
use agent_desktop_core::{
    AdapterError, Deadline, DeliveryDisposition, ErrorCode, RetryDisposition,
};
use std::cell::RefCell;

struct ScriptedEdit {
    select: Result<(), EditSendFailure>,
    replace: Result<(), EditSendFailure>,
    sent: RefCell<Vec<(isize, EditMessage)>>,
}

impl ScriptedEdit {
    fn answering(
        select: Result<(), EditSendFailure>,
        replace: Result<(), EditSendFailure>,
    ) -> Self {
        Self {
            select,
            replace,
            sent: RefCell::new(Vec::new()),
        }
    }

    fn sent(&self) -> Vec<(isize, EditMessage)> {
        self.sent.borrow().clone()
    }
}

impl EditMessenger for ScriptedEdit {
    fn send(
        &self,
        handle: isize,
        message: EditMessage,
        _deadline: Deadline,
    ) -> Result<(), EditSendFailure> {
        let answer = match message {
            EditMessage::SelectAll => self.select.clone(),
            EditMessage::ReplaceSelection(_) => self.replace.clone(),
        };
        self.sent.borrow_mut().push((handle, message));
        answer
    }
}

fn deadline() -> Deadline {
    Deadline::after(1_000).expect("deadline")
}

fn unanswered() -> EditSendFailure {
    EditSendFailure::Unanswered("timed out".into())
}

fn not_sent() -> EditSendFailure {
    EditSendFailure::NotSent("refused".into())
}

fn utf16_with_nul(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn assert_disposition(
    error: &AdapterError,
    delivery: DeliveryDisposition,
    retry: RetryDisposition,
) {
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition.delivery(), delivery);
    assert_eq!(error.disposition.retry(), retry);
}

#[test]
fn an_answered_selection_and_replace_write_the_value_with_its_terminator() {
    let edit = ScriptedEdit::answering(Ok(()), Ok(()));
    replace_text(&edit, 7, "naïve ✓", deadline()).expect("both messages answered");
    assert_eq!(
        edit.sent(),
        vec![
            (7, EditMessage::SelectAll),
            (7, EditMessage::ReplaceSelection(utf16_with_nul("naïve ✓"))),
        ]
    );
}

#[test]
fn an_unanswered_selection_is_not_delivered_and_sends_no_text() {
    let edit = ScriptedEdit::answering(Err(unanswered()), Ok(()));
    let error = replace_text(&edit, 7, "value", deadline()).expect_err("selection timed out");
    assert_disposition(
        &error,
        DeliveryDisposition::NotDelivered,
        RetryDisposition::Safe,
    );
    assert_eq!(edit.sent(), vec![(7, EditMessage::SelectAll)]);
    assert!(
        error
            .platform_detail
            .as_deref()
            .is_some_and(|detail| detail.starts_with("EM_SETSEL")),
        "{error:?}"
    );
}

#[test]
fn a_selection_that_was_never_sent_is_not_delivered_and_sends_no_text() {
    let edit = ScriptedEdit::answering(Err(not_sent()), Ok(()));
    let error = replace_text(&edit, 7, "value", deadline()).expect_err("selection refused");
    assert_disposition(
        &error,
        DeliveryDisposition::NotDelivered,
        RetryDisposition::Safe,
    );
    assert_eq!(edit.sent(), vec![(7, EditMessage::SelectAll)]);
}

#[test]
fn an_unanswered_replace_may_still_land_so_it_is_delivered_unverified() {
    let edit = ScriptedEdit::answering(Ok(()), Err(unanswered()));
    let error = replace_text(&edit, 7, "value", deadline()).expect_err("replace timed out");
    assert_disposition(
        &error,
        DeliveryDisposition::DeliveredUnverified,
        RetryDisposition::Unsafe,
    );
    assert_eq!(edit.sent().len(), 2);
    assert!(
        error
            .platform_detail
            .as_deref()
            .is_some_and(|detail| detail.starts_with("EM_REPLACESEL")),
        "{error:?}"
    );
}

#[test]
fn a_replace_that_never_reached_the_edit_is_not_delivered() {
    let edit = ScriptedEdit::answering(Ok(()), Err(not_sent()));
    let error = replace_text(&edit, 7, "value", deadline()).expect_err("replace refused");
    assert_disposition(
        &error,
        DeliveryDisposition::NotDelivered,
        RetryDisposition::Safe,
    );
    assert_eq!(edit.sent().len(), 2);
}
