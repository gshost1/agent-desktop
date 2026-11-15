use super::{EditMessenger, EditSendFailure, replace_text};
use agent_desktop_core::{
    AdapterError, Deadline, DeliveryDisposition, ErrorCode, RetryDisposition,
};
use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Sent {
    Reserve(isize),
    SelectAll(isize),
    Replace(isize, Vec<u16>),
}

struct ScriptedEdit {
    reserve: Result<(), EditSendFailure>,
    select: Result<(), EditSendFailure>,
    replace: Result<(), EditSendFailure>,
    sent: RefCell<Vec<Sent>>,
}

impl ScriptedEdit {
    fn answering(
        select: Result<(), EditSendFailure>,
        replace: Result<(), EditSendFailure>,
    ) -> Self {
        Self {
            reserve: Ok(()),
            select,
            replace,
            sent: RefCell::new(Vec::new()),
        }
    }

    fn refusing_the_reservation() -> Self {
        Self {
            reserve: Err(not_sent()),
            ..Self::answering(Ok(()), Ok(()))
        }
    }

    fn sent(&self) -> Vec<Sent> {
        self.sent.borrow().clone()
    }
}

impl EditMessenger for ScriptedEdit {
    type Reservation = isize;

    fn reserve(&self, handle: isize) -> Result<isize, EditSendFailure> {
        self.sent.borrow_mut().push(Sent::Reserve(handle));
        self.reserve.clone().map(|()| handle)
    }

    fn select_all(&self, handle: isize, _deadline: Deadline) -> Result<(), EditSendFailure> {
        self.sent.borrow_mut().push(Sent::SelectAll(handle));
        self.select.clone()
    }

    fn replace_selection(
        &self,
        reservation: isize,
        text: Vec<u16>,
        _deadline: Deadline,
    ) -> Result<(), EditSendFailure> {
        self.sent
            .borrow_mut()
            .push(Sent::Replace(reservation, text));
        self.replace.clone()
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

fn detail_starts_with(error: &AdapterError, prefix: &str) -> bool {
    error
        .platform_detail
        .as_deref()
        .is_some_and(|detail| detail.starts_with(prefix))
}

#[test]
fn an_answered_write_reserves_selects_then_replaces_with_the_terminated_value() {
    let edit = ScriptedEdit::answering(Ok(()), Ok(()));
    replace_text(&edit, 7, "naïve ✓", deadline()).expect("every step answered");
    assert_eq!(
        edit.sent(),
        vec![
            Sent::Reserve(7),
            Sent::SelectAll(7),
            Sent::Replace(7, utf16_with_nul("naïve ✓")),
        ]
    );
}

#[test]
fn a_refused_reservation_is_not_delivered_and_sends_nothing() {
    let edit = ScriptedEdit::refusing_the_reservation();
    let error = replace_text(&edit, 7, "value", deadline()).expect_err("reservation refused");
    assert_disposition(
        &error,
        DeliveryDisposition::NotDelivered,
        RetryDisposition::Safe,
    );
    assert_eq!(
        edit.sent(),
        vec![Sent::Reserve(7)],
        "not even the selection"
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
    assert_eq!(edit.sent(), vec![Sent::Reserve(7), Sent::SelectAll(7)]);
    assert!(detail_starts_with(&error, "EM_SETSEL"), "{error:?}");
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
    assert_eq!(edit.sent(), vec![Sent::Reserve(7), Sent::SelectAll(7)]);
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
    assert_eq!(edit.sent().len(), 3);
    assert!(detail_starts_with(&error, "EM_REPLACESEL"), "{error:?}");
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
    assert_eq!(edit.sent().len(), 3);
}
