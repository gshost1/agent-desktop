use agent_desktop_core::{DeliveryDisposition, StepMechanism};

use super::*;

struct Rows {
    expanded: Option<bool>,
    satisfied: Option<bool>,
    opens: bool,
    failure: Option<&'static str>,
    operations: Vec<&'static str>,
}

impl Rows {
    fn closed() -> Self {
        Self {
            expanded: Some(false),
            satisfied: Some(false),
            opens: true,
            failure: None,
            operations: Vec::new(),
        }
    }
}

impl ComboRows for Rows {
    fn satisfied(&mut self) -> Option<bool> {
        self.satisfied
    }

    fn expanded(&mut self, _deadline: Deadline) -> Option<bool> {
        if self.failure == Some("unread") && self.operations.contains(&"open") {
            None
        } else {
            self.expanded
        }
    }

    fn open(&mut self) -> Result<Vec<ActionStep>, AdapterError> {
        self.operations.push("open");
        if self.failure == Some("open") {
            return Err(AdapterError::new(ErrorCode::StaleRef, "opener refused")
                .with_disposition(DeliverySemantics::not_delivered()));
        }
        self.expanded = Some(self.opens);
        Ok(vec![
            ActionStep::succeeded("open")
                .with_mechanism(StepMechanism::PhysicalSynthetic)
                .with_verified(false),
        ])
    }

    fn choose(&mut self) -> Result<ActionResult, AdapterError> {
        self.operations.push("choose");
        assert_eq!(self.expanded, Some(true));
        if self.failure == Some("choose") {
            return Err(
                AdapterError::new(ErrorCode::ElementNotFound, "choice disappeared")
                    .with_disposition(DeliverySemantics::not_delivered()),
            );
        }
        Ok(ActionResult::delivered_unverified("select")
            .with_steps(vec![ActionStep::succeeded("choose").with_verified(true)])
            .with_verified_delivery())
    }

    fn close(&mut self, _deadline: Deadline) -> bool {
        self.operations.push("escape");
        self.failure != Some("close")
    }
}

fn select(rows: &mut Rows) -> Result<ActionResult, AdapterError> {
    choose(rows, Deadline::after(1000).expect("test deadline"))
}

#[test]
fn a_closed_combo_is_opened_before_finding_and_clicking_the_choice() {
    let mut rows = Rows::closed();
    let result = select(&mut rows).expect("selected");
    assert_eq!(rows.operations, ["open", "choose"]);
    assert_eq!(result.steps.len(), 2);
    assert_eq!(result.steps[0].verified(), Some(false));
    assert_eq!(result.steps[1].verified(), Some(true));
    assert_eq!(
        result.disposition().delivery(),
        DeliveryDisposition::DeliveredVerified
    );
}

#[test]
fn an_open_combo_is_not_clicked_shut_before_selecting() {
    let mut rows = Rows {
        expanded: Some(true),
        ..Rows::closed()
    };
    let result = select(&mut rows).expect("selected");
    assert_eq!(rows.operations, ["choose"]);
    assert_eq!(result.steps.len(), 1);
}

#[test]
fn an_already_shown_choice_needs_no_pointer_input_or_expansion_read() {
    let mut rows = Rows {
        satisfied: Some(true),
        expanded: None,
        ..Rows::closed()
    };
    let result = select(&mut rows).expect("already selected");
    assert!(rows.operations.is_empty());
    assert_eq!(
        result.disposition().delivery(),
        DeliveryDisposition::NotDelivered
    );
}

#[test]
fn an_unread_expansion_state_never_guesses_whether_to_toggle() {
    let mut rows = Rows {
        expanded: None,
        ..Rows::closed()
    };
    let error = select(&mut rows).expect_err("unread expansion");
    assert!(rows.operations.is_empty());
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(
        error.details.expect("details")["kind"],
        "expansion_state_unread"
    );
}

#[test]
fn a_combo_that_never_advertises_expansion_is_not_assumed_closed() {
    assert_eq!(observed_expansion(Some(false), Some(false)), None);
    assert_eq!(observed_expansion(Some(false), None), None);
    assert_eq!(observed_expansion(None, Some(true)), None);
    assert_eq!(observed_expansion(Some(false), Some(true)), Some(false));
    assert_eq!(observed_expansion(Some(true), Some(false)), Some(true));
}

#[test]
fn a_refused_opener_does_not_try_the_invisible_row() {
    let mut rows = Rows {
        failure: Some("open"),
        ..Rows::closed()
    };
    let error = select(&mut rows).expect_err("opener refused");
    assert_eq!(rows.operations, ["open"]);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn absent_or_unread_expansion_after_opening_closes_the_popup_without_clicking_a_row() {
    for failure in [None, Some("unread")] {
        let mut rows = Rows {
            opens: false,
            failure,
            ..Rows::closed()
        };
        let error = select(&mut rows).expect_err("popup not observed");
        assert_eq!(rows.operations, ["open", "escape"]);
        assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
        let details = error.details.expect("details");
        assert_eq!(details["kind"], "popup_not_observed");
        assert_eq!(details["popup_closed"], true);
    }
}

#[test]
fn a_close_failure_keeps_the_original_unsafe_error_and_does_not_claim_restoration() {
    let mut rows = Rows {
        opens: false,
        failure: Some("close"),
        ..Rows::closed()
    };
    let error = select(&mut rows).expect_err("popup not observed");
    assert_eq!(rows.operations, ["open", "escape"]);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.expect("details");
    assert_eq!(details["kind"], "popup_not_observed");
    assert_eq!(details["popup_closed"], false);
}

#[test]
fn an_insufficient_cleanup_budget_refuses_before_opening() {
    let mut rows = Rows::closed();
    let error =
        choose(&mut rows, Deadline::after(1).expect("deadline")).expect_err("no cleanup budget");
    assert!(rows.operations.is_empty());
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(
        error.details.expect("details")["kind"],
        "popup_cleanup_budget_insufficient"
    );
}

#[test]
fn a_choice_missing_after_opening_is_not_reported_as_untouched() {
    let mut rows = Rows {
        failure: Some("choose"),
        ..Rows::closed()
    };
    let error = select(&mut rows).expect_err("choice disappeared");
    assert_eq!(rows.operations, ["open", "choose"]);
    assert_eq!(error.code, ErrorCode::ElementNotFound);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}

#[test]
fn a_choice_refused_in_an_already_open_popup_keeps_its_safe_disposition() {
    let mut rows = Rows {
        expanded: Some(true),
        failure: Some("choose"),
        ..Rows::closed()
    };
    let error = select(&mut rows).expect_err("choice refused");
    assert_eq!(rows.operations, ["choose"]);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn a_row_visibility_refusal_after_opening_accounts_for_the_opener_input() {
    let error = AdapterError::new(ErrorCode::StaleRef, "row not visible")
        .with_details(
            json!({"kind": "pointer_target_not_visible", "delivered_clicks": 0,
                            "physical_delivery_started": false}),
        )
        .with_disposition(DeliverySemantics::not_delivered());
    let error = after_opener(error);
    let details = error.details.expect("details");
    assert_eq!(details["delivered_clicks"], 1);
    assert_eq!(details["physical_delivery_started"], true);
    assert_eq!(details["combo_opener_clicked"], true);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}
