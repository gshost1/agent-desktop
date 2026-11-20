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
        assert_eq!(details["popup_closed"], true);
        assert_eq!(details["combo_opener_clicked"], true);
    }
}
