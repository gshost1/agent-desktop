use super::*;

struct TimedChoices {
    inner: Fake,
    deadline: Deadline,
    original: Deadline,
    exhaust: &'static str,
    budgets: Vec<Deadline>,
}

impl TimedChoices {
    fn spend_budget(&self) -> AdapterError {
        assert_ne!(self.deadline, self.original);
        std::thread::sleep(self.deadline.remaining() + Duration::from_millis(2));
        assert!(self.deadline.is_expired());
        assert!(!self.original.is_expired());
        self.deadline
            .timeout_error()
            .with_disposition(DeliverySemantics::not_delivered())
    }
}

impl Choices for TimedChoices {
    fn set_deadline(&mut self, deadline: Deadline) {
        self.deadline = deadline;
        self.budgets.push(deadline);
    }

    fn lists(&mut self) -> Result<Vec<AtspiElement>, AdapterError> {
        self.deadline.ensure_remaining()?;
        if self.exhaust == "popup" && self.inner.0.borrow().open && self.deadline != self.original {
            return Err(self.spend_budget());
        }
        self.inner.lists()
    }

    fn shown(&mut self, current: &AtspiElement) -> Option<String> {
        self.inner.shown(current)
    }

    fn find(
        &mut self,
        popup: &AtspiElement,
        wanted: &str,
    ) -> Result<Search<AtspiElement>, AdapterError> {
        assert_ne!(self.deadline, self.original);
        if self.exhaust == "find" {
            return Err(self.spend_budget());
        }
        self.deadline.ensure_remaining()?;
        self.inner.find(popup, wanted)
    }

    fn commit(
        &mut self,
        hit: &Hit<AtspiElement>,
        current: &AtspiElement,
        wanted: &str,
    ) -> Result<ActionResult, AdapterError> {
        assert_ne!(self.deadline, self.original);
        if self.exhaust == "commit" {
            return Err(self.spend_budget());
        }
        self.deadline.ensure_remaining()?;
        self.inner.commit(hit, current, wanted)
    }

    fn overview_open(&self) -> Option<bool> {
        self.inner.overview_open()
    }
}

struct TimedPointer {
    inner: Fake,
    deadline: Deadline,
}

impl Pointer for TimedPointer {
    fn click(&self, element: &AtspiElement) -> Result<Vec<ActionStep>, AdapterError> {
        self.deadline.ensure_remaining()?;
        self.inner.click(element)
    }

    fn escape(&self) -> Result<Vec<ActionStep>, AdapterError> {
        self.deadline.ensure_remaining()?;
        self.inner.escape()
    }
}

#[test]
fn exhausted_popup_find_and_commit_budgets_leave_live_reads_and_escape_for_cleanup() {
    for exhaust in ["popup", "find", "commit"] {
        let world = World::dropdown();
        let deadline = Deadline::after(1300).expect("deadline with close reserve");
        let mut choices = TimedChoices {
            inner: Fake(world.clone()),
            deadline,
            original: deadline,
            exhaust,
            budgets: Vec::new(),
        };
        let pointer = TimedPointer {
            inner: Fake(world.clone()),
            deadline,
        };
        let error = choose_by_pointer(
            &mut choices,
            Some(&pointer),
            &element("/combo"),
            "Never",
            deadline,
        )
        .expect_err("work budget spent");
        assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
        assert_eq!(
            error.code,
            if exhaust == "popup" {
                ErrorCode::ActionFailed
            } else {
                ErrorCode::Timeout
            }
        );
        let details = error.details.expect("details");
        assert_eq!(details["popup_closed"], true, "{exhaust}");
        assert_eq!(details["popup_close_attempted"], true, "{exhaust}");
        assert_eq!(choices.budgets.len(), 2);
        assert!(choices.budgets[0].is_expired());
        assert_eq!(choices.budgets[1], deadline);
        assert!(!deadline.is_expired());
        assert!(!world.borrow().open);
        assert_eq!(world.borrow().escapes, 1);
        assert_eq!(world.borrow().clicks, ["/combo"]);
    }
}

#[test]
fn insufficient_cleanup_budget_refuses_before_opening_the_combo() {
    let world = World::dropdown();
    let error = choose_by_pointer(
        &mut Fake(world.clone()),
        Some(&Fake(world.clone())),
        &element("/combo"),
        "Never",
        Deadline::after(1000).expect("only enough time to close"),
    )
    .expect_err("no budget to open");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(
        error.details.expect("details")["kind"],
        "popup_cleanup_budget_insufficient"
    );
    assert!(world.borrow().clicks.is_empty());
    assert_eq!(world.borrow().escapes, 0);
}
