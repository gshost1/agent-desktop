use super::*;

struct ReadFailure {
    inner: Fake,
    code: ErrorCode,
    hidden: Option<bool>,
    failed: bool,
}

impl Choices for ReadFailure {
    fn lists(&mut self) -> Result<Vec<AtspiElement>, AdapterError> {
        if self.failed && self.hidden.is_none() {
            return Err(AdapterError::new(ErrorCode::Timeout, "popup state unread"));
        }
        self.inner.lists()
    }

    fn shown(&mut self, current: &AtspiElement) -> Option<String> {
        self.inner.shown(current)
    }

    fn find(&mut self, _: &AtspiElement, _: &str) -> Result<Search<AtspiElement>, AdapterError> {
        self.failed = true;
        if self.hidden == Some(true) {
            self.inner.0.borrow_mut().open = false;
        }
        Err(AdapterError::new(self.code.clone(), "popup read failed")
            .with_details(json!({"kind": "popup_read_failed", "read_marker": 17}))
            .with_disposition(DeliverySemantics::not_delivered()))
    }

    fn commit(
        &mut self,
        _: &Hit<AtspiElement>,
        _: &AtspiElement,
        _: &str,
    ) -> Result<ActionResult, AdapterError> {
        panic!("a failed read cannot commit a choice")
    }

    fn overview_open(&self) -> Option<bool> {
        self.inner.overview_open()
    }
}

fn refuse_read(
    world: &Rc<RefCell<World>>,
    pointer: &dyn Pointer,
    code: ErrorCode,
    hidden: Option<bool>,
) -> AdapterError {
    choose_by_pointer(
        &mut ReadFailure {
            inner: Fake(world.clone()),
            code,
            hidden,
            failed: false,
        },
        Some(pointer),
        &element("/combo"),
        "Never",
        Deadline::after(300).expect("deadline"),
    )
    .expect_err("popup read refused")
}

#[test]
fn popup_read_and_deadline_errors_close_the_popup_and_keep_the_opener_disposition() {
    for code in [ErrorCode::ActionFailed, ErrorCode::Timeout] {
        let world = World::dropdown();
        let error = refuse_read(&world, &Fake(world.clone()), code.clone(), Some(false));
        assert_eq!(error.code, code);
        assert_eq!(error.message, "popup read failed");
        assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
        let details = error.details.expect("details");
        assert_eq!(details["kind"], "popup_read_failed");
        assert_eq!(details["read_marker"], 17);
        assert_eq!(details["popup_close_attempted"], true);
        assert_eq!(details["popup_closed"], true);
        let world = world.borrow();
        assert_eq!(world.clicks, ["/combo"]);
        assert_eq!(world.escapes, 1);
        assert!(world.commits.is_empty());
        assert!(!world.open);
    }
}

#[test]
fn a_hidden_or_unread_popup_after_a_read_error_is_not_sent_escape() {
    for hidden in [Some(true), None] {
        let world = World::dropdown();
        let error = refuse_read(&world, &Fake(world.clone()), ErrorCode::Timeout, hidden);
        assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
        let details = error.details.expect("details");
        assert_eq!(details["popup_close_attempted"], false);
        assert_eq!(details["popup_closed"], json!(hidden));
        assert_eq!(world.borrow().escapes, 0);
        assert!(world.borrow().commits.is_empty());
    }
}

#[test]
fn a_read_error_whose_escape_does_not_close_the_popup_keeps_the_original_error() {
    let world = World::dropdown();
    world.borrow_mut().escape_closes = false;
    let error = refuse_read(
        &world,
        &Fake(world.clone()),
        ErrorCode::Timeout,
        Some(false),
    );
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.expect("details");
    assert_eq!(details["popup_closed"], false);
    assert_eq!(details["popup_close_attempted"], true);
    assert_eq!(world.borrow().escapes, 1);
    assert!(world.borrow().open);
}

struct RefusedEscape(Fake);

impl Pointer for RefusedEscape {
    fn click(&self, target: &AtspiElement) -> Result<Vec<ActionStep>, AdapterError> {
        self.0.click(target)
    }

    fn escape(&self) -> Result<Vec<ActionStep>, AdapterError> {
        Err(AdapterError::new(ErrorCode::PolicyDenied, "escape refused")
            .with_disposition(DeliverySemantics::not_delivered()))
    }
}

#[test]
fn a_refused_escape_keeps_the_read_error_and_does_not_claim_input_or_closure() {
    let world = World::dropdown();
    let pointer = RefusedEscape(Fake(world.clone()));
    let error = refuse_read(&world, &pointer, ErrorCode::Timeout, Some(false));
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.expect("details");
    assert_eq!(details["popup_close_attempted"], false);
    assert!(details["popup_closed"].is_null());
    assert_eq!(world.borrow().escapes, 0);
    assert!(world.borrow().open);
}
