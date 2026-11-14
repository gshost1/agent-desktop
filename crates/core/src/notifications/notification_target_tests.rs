use super::NotificationTarget;
use crate::notifications::notification_token::NotificationToken;
use crate::{DeliverySemantics, ErrorCode, NotificationIdentity, NotificationInfo};

fn info(index: usize, app: &str, title: &str, body: &str) -> NotificationInfo {
    NotificationInfo {
        index,
        app_name: app.into(),
        title: title.into(),
        body: Some(body.into()),
        actions: vec![],
    }
}

fn key_of(listed: &[NotificationInfo], at: usize) -> String {
    NotificationToken::assign(listed)[at].to_string()
}

fn app(name: &str) -> NotificationIdentity {
    NotificationIdentity {
        expected_app: Some(name.into()),
        expected_title: None,
    }
}

fn seen() -> Vec<NotificationInfo> {
    vec![
        info(1, "Slack", "New message", "from Ann"),
        info(2, "Mail", "Invoice", "due"),
    ]
}

fn shifted() -> Vec<NotificationInfo> {
    vec![
        info(1, "Slack", "New message", "from Bob"),
        info(2, "Slack", "New message", "from Ann"),
        info(3, "Mail", "Invoice", "due"),
    ]
}

#[test]
fn a_key_follows_its_entry_when_another_same_app_entry_arrives_above() {
    let target =
        NotificationTarget::new(None, Some(&key_of(&seen(), 0)), app("Slack")).expect("target");
    let listed = shifted();
    let at = target.pick(&listed).expect("found");
    assert_eq!(listed[at].body.as_deref(), Some("from Ann"));
}

#[test]
fn a_key_pinned_to_an_index_refuses_when_the_list_shifted() {
    let target = NotificationTarget::new(
        Some(1),
        Some(&key_of(&seen(), 0)),
        NotificationIdentity::default(),
    )
    .expect("target");
    let error = target.pick(&shifted()).expect_err("index 1 is Bob's now");
    assert_eq!(error.code, ErrorCode::NotificationNotFound);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(target.pick(&seen()).is_ok());
}

#[test]
fn a_gone_key_is_refused_not_delivered() {
    let target = NotificationTarget::new(
        None,
        Some(&key_of(&seen(), 0)),
        NotificationIdentity::default(),
    )
    .expect("target");
    let error = target.pick(&seen()[1..]).expect_err("Ann's message left");
    assert_eq!(error.code, ErrorCode::NotificationNotFound);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(error.message.contains("No listed notification has key"));
}

#[test]
fn identical_twins_are_addressed_separately_by_occurrence() {
    let twins = vec![
        info(1, "CI", "Build failed", "main"),
        info(2, "CI", "Build failed", "main"),
    ];
    let first = NotificationTarget::new(
        None,
        Some(&key_of(&twins, 0)),
        NotificationIdentity::default(),
    )
    .expect("target");
    let second = NotificationTarget::new(
        None,
        Some(&key_of(&twins, 1)),
        NotificationIdentity::default(),
    )
    .expect("target");
    assert_eq!(first.pick(&twins).expect("first"), 0);
    assert_eq!(second.pick(&twins).expect("second"), 1);
    let error = first.pick(&twins[1..]).expect_err("the newer twin left");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn a_key_whose_older_twin_left_is_refused_instead_of_naming_the_newer_twin() {
    let twins = vec![
        info(1, "CI", "Build failed", "main"),
        info(2, "CI", "Build failed", "main"),
    ];
    let oldest = NotificationTarget::new(
        None,
        Some(&key_of(&twins, 1)),
        NotificationIdentity::default(),
    )
    .expect("target");
    let error = oldest
        .pick(&twins[..1])
        .expect_err("the remaining twin now carries the oldest's number");
    assert_eq!(error.code, ErrorCode::NotificationNotFound);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(error.message.contains("identical"), "{}", error.message);
}

#[test]
fn expected_fields_beside_a_key_must_still_match() {
    let target =
        NotificationTarget::new(None, Some(&key_of(&seen(), 0)), app("Mail")).expect("target");
    let error = target
        .pick(&seen())
        .expect_err("the keyed entry is Slack's");
    assert_eq!(error.code, ErrorCode::NotificationNotFound);
}

#[test]
fn an_index_with_expected_fields_refuses_an_entry_that_does_not_carry_them() {
    let target = NotificationTarget::new(Some(3), None, app("Slack")).expect("target");
    let error = target.pick(&shifted()).expect_err("index 3 is Mail's");
    assert_eq!(error.code, ErrorCode::NotificationNotFound);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(!error.message.contains("Mail"));
    assert_eq!(
        NotificationTarget::new(Some(2), None, app("Slack"))
            .expect("t")
            .pick(&shifted())
            .expect("ok"),
        1
    );
}

#[test]
fn an_index_that_is_no_longer_listed_is_not_found() {
    let target = NotificationTarget::new(Some(9), None, app("Slack")).expect("target");
    let error = target.pick(&shifted()).expect_err("no ninth entry");
    assert_eq!(error.code, ErrorCode::NotificationNotFound);
}

#[test]
fn an_identical_newcomer_at_the_top_leaves_the_listed_twin_addressed() {
    let seen = vec![
        info(1, "CI", "Build failed", "main"),
        info(2, "Mail", "Invoice", "due"),
    ];
    let target = NotificationTarget::new(
        None,
        Some(&key_of(&seen, 0)),
        NotificationIdentity::default(),
    )
    .expect("target");
    let now = vec![
        info(1, "CI", "Build failed", "main"),
        info(2, "CI", "Build failed", "main"),
        info(3, "Mail", "Invoice", "due"),
    ];
    assert_eq!(target.pick(&now).expect("the original"), 1);
}

#[test]
fn the_app_scope_is_checked_after_the_pick_with_the_mismatch_refusal() {
    let key = key_of(&seen(), 0);
    let scoped = |app: &str| {
        NotificationTarget::new(None, Some(&key), NotificationIdentity::default())
            .expect("target")
            .with_app(Some(app.into()))
    };
    assert_eq!(scoped("SLA").pick(&shifted()).expect("in scope"), 1);
    let error = scoped("mail").pick(&shifted()).expect_err("out of scope");
    assert_eq!(error.code, ErrorCode::NotificationNotFound);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(
        error.message,
        "The notification at index 2 does not match the expected app or title"
    );
    let expected = NotificationTarget::new(None, Some(&key), app("Mail")).expect("target");
    assert_eq!(
        expected.pick(&shifted()).expect_err("fields").message,
        error.message
    );
}

#[test]
fn an_index_with_one_empty_and_one_set_field_checks_only_the_set_one() {
    let mixed = NotificationIdentity {
        expected_app: Some(String::new()),
        expected_title: Some("Invoice".into()),
    };
    let target = NotificationTarget::new(Some(3), None, mixed).expect("one field is set");
    assert_eq!(target.pick(&shifted()).expect("title matches"), 2);
    let wrong = NotificationIdentity {
        expected_app: Some(String::new()),
        expected_title: Some("Reminder".into()),
    };
    let target = NotificationTarget::new(Some(3), None, wrong).expect("target");
    assert!(target.pick(&shifted()).is_err());
}

#[test]
fn an_index_with_both_fields_needs_both_to_match() {
    let both = NotificationIdentity {
        expected_app: Some("Mail".into()),
        expected_title: Some("Invoice".into()),
    };
    let target = NotificationTarget::new(Some(3), None, both).expect("target");
    assert!(target.pick(&shifted()).is_ok());
    let other = NotificationIdentity {
        expected_app: Some("Slack".into()),
        expected_title: Some("Invoice".into()),
    };
    let target = NotificationTarget::new(Some(3), None, other).expect("target");
    assert!(target.pick(&shifted()).is_err());
}

#[test]
fn check_applies_the_same_addressing_rule_as_new() {
    assert!(NotificationTarget::check(true, None, &app("Slack")).is_ok());
    assert!(NotificationTarget::check(false, Some(&key_of(&seen(), 0)), &app("")).is_ok());
    for (has_index, key, identity) in [
        (true, None, app("")),
        (false, None, app("Slack")),
        (false, Some("bad"), app("")),
    ] {
        let error = NotificationTarget::check(has_index, key, &identity).expect_err("refused");
        assert_eq!(error.code, ErrorCode::InvalidArgs);
    }
}

#[test]
fn a_bare_index_and_a_malformed_key_are_invalid_args() {
    let bare =
        NotificationTarget::new(Some(1), None, NotificationIdentity::default()).expect_err("bare");
    assert_eq!(bare.code, ErrorCode::InvalidArgs);
    let empty = NotificationTarget::new(Some(1), None, app("")).expect_err("empty identity");
    assert_eq!(empty.code, ErrorCode::InvalidArgs);
    let nothing = NotificationTarget::new(None, None, app("Slack")).expect_err("no index");
    assert_eq!(nothing.code, ErrorCode::InvalidArgs);
    let malformed = NotificationTarget::new(None, Some("3"), NotificationIdentity::default())
        .expect_err("bad key");
    assert_eq!(malformed.code, ErrorCode::InvalidArgs);
}
