use super::*;

#[test]
fn choice_failures_close_only_a_popup_still_observed_as_visible() {
    for (failure, hidden, sent, closed) in [
        ("choose", Some(false), true, Some(true)),
        ("close", Some(false), true, Some(false)),
        ("close-unread", Some(false), true, None),
        ("choose", Some(true), false, None),
        ("choose", None, false, None),
        ("hidden-before-close", Some(false), false, None),
        ("unread-before-close", Some(false), false, None),
        ("escape-refused", Some(false), false, None),
    ] {
        let mut rows = Rows {
            failure: Some(failure),
            hidden,
            ..Rows::closed()
        };
        let error = select(&mut rows).expect_err("choice refused");
        let expected: &[&str] = if sent {
            &["open", "choose", "escape"]
        } else {
            &["open", "choose"]
        };
        assert_eq!(rows.operations, expected, "{failure}");
        assert_eq!(error.code, ErrorCode::ElementNotFound);
        assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
        let details = error.details.expect("details");
        assert_eq!(details["kind"], "choice_missing");
        assert_eq!(details["popup_close_attempted"], sent, "{failure}");
        assert_eq!(details["popup_closed"], json!(closed), "{failure}");
        assert_eq!(details["delivered_clicks"], 1);
        assert_eq!(details["physical_delivery_started"], true);
        assert_eq!(details["combo_opener_clicked"], true);
    }
}

#[test]
fn ambiguous_and_ignored_choices_close_the_popup_and_keep_click_accounting() {
    for (failure, code, clicks) in [
        ("row-ambiguous", ErrorCode::AmbiguousTarget, 1),
        ("click-ignored", ErrorCode::ActionFailed, 2),
    ] {
        let mut rows = Rows {
            failure: Some(failure),
            ..Rows::closed()
        };
        let error = select(&mut rows).expect_err("choice refused");
        assert_eq!(rows.operations, ["open", "choose", "escape"]);
        assert_eq!(error.code, code);
        assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
        let details = error.details.expect("details");
        assert_eq!(details["popup_closed"], true);
        assert_eq!(details["popup_close_attempted"], true);
        assert_eq!(details["delivered_clicks"], clicks);
        assert_eq!(details["physical_delivery_started"], true);
        assert_eq!(details["combo_opener_clicked"], true);
    }
}

#[test]
fn an_unobserved_qt_popup_shares_the_overview_cause_and_recovery_advice() {
    for (failure, overview) in [
        ("overview-open", Some(true)),
        ("overview-closed", Some(false)),
        ("overview-unread", None),
    ] {
        let mut rows = Rows {
            opens: false,
            failure: Some(failure),
            hidden: Some(true),
            ..Rows::closed()
        };
        let error = select(&mut rows).expect_err("popup not observed");
        let expected = popup_not_observed(overview);
        assert_eq!(error.suggestion, expected.suggestion);
        assert!(error.suggestion.is_some());
        assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
        let details = error.details.expect("details");
        assert_eq!(details["kind"], "popup_not_observed");
        if overview == Some(true) {
            assert_eq!(details["cause"], "overview_open");
        } else {
            assert!(details.get("cause").is_none());
        }
        assert_eq!(rows.operations, ["open"]);
        assert_eq!(details["popup_close_attempted"], false);
        assert!(details["popup_closed"].is_null());
        assert_eq!(details["combo_opener_clicked"], true);
    }
}

#[test]
fn a_visible_or_unread_popup_does_not_read_or_name_the_overview() {
    for hidden in [Some(false), None] {
        let mut rows = Rows {
            opens: false,
            failure: Some("overview-open"),
            hidden,
            ..Rows::closed()
        };
        let error = select(&mut rows).expect_err("expansion not observed");
        assert_eq!(error.suggestion, popup_not_observed(None).suggestion);
        let details = error.details.expect("details");
        assert!(details.get("cause").is_none());
        assert_eq!(details["popup_close_attempted"], hidden == Some(false));
    }
}

struct BudgetRows {
    rows: Rows,
    chosen: Option<Deadline>,
    cleanup: Cell<Option<Deadline>>,
}

impl ComboRows for BudgetRows {
    fn satisfied(&mut self) -> Option<bool> {
        self.rows.satisfied()
    }

    fn expanded(&mut self, deadline: Deadline) -> Option<bool> {
        self.rows.expanded(deadline)
    }

    fn open(&mut self) -> Result<Vec<ActionStep>, AdapterError> {
        self.rows.open()
    }

    fn popup_hidden(&self, deadline: Deadline) -> Option<bool> {
        self.cleanup.set(Some(deadline));
        self.rows.popup_hidden(deadline)
    }

    fn escape(&mut self) -> Option<()> {
        self.rows.escape()
    }

    fn overview_open(&self) -> Option<bool> {
        self.rows.overview_open()
    }

    fn choose(&mut self, deadline: Deadline) -> Result<ActionResult, AdapterError> {
        self.chosen = Some(deadline);
        self.rows.choose(deadline)
    }
}

#[test]
fn choosing_after_opening_keeps_the_close_reservation_and_cleanup_uses_the_full_deadline() {
    for expanded in [false, true] {
        let deadline = Deadline::after(5_000).expect("deadline");
        let mut rows = BudgetRows {
            rows: Rows {
                expanded: Some(expanded),
                failure: Some("choose"),
                ..Rows::closed()
            },
            chosen: None,
            cleanup: Cell::new(None),
        };
        choose(&mut rows, deadline).expect_err("choice refused");
        let chosen = rows.chosen.expect("selection deadline");
        if expanded {
            assert_eq!(chosen, deadline);
            assert_eq!(rows.cleanup.get(), None);
        } else {
            assert_ne!(chosen, deadline);
            assert!(
                deadline.remaining().saturating_sub(chosen.remaining())
                    >= CLOSE_WAIT.saturating_sub(Duration::from_millis(10))
            );
            assert_eq!(rows.cleanup.get(), Some(deadline));
            assert_eq!(rows.rows.operations, ["open", "choose", "escape"]);
        }
    }
}
