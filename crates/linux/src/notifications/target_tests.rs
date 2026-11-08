use super::*;
use crate::notifications::fake::{FakeWorld, log, reading, spec};
use agent_desktop_core::{Deadline, ErrorCode, NotificationIdentity};

fn deadline() -> Deadline {
    Deadline::after(1_000).expect("a deadline")
}

fn at_one(app: &str) -> NotificationTarget {
    let identity = NotificationIdentity {
        expected_app: Some(app.to_owned()),
        expected_title: None,
    };
    NotificationTarget::new(Some(1), None, identity).expect("target")
}

fn wanted(app_filter: Option<&str>, target: &NotificationTarget) -> NotificationTarget {
    target.clone().with_app(app_filter.map(String::from))
}

#[test]
fn an_entry_carrying_the_expected_app_is_located() {
    let world = FakeWorld::new(&log(), vec![reading(&[spec("A", "one")])]);
    let any = at_one("A");
    assert!(locate(&world, &wanted(None, &any), deadline()).is_ok());
}

#[test]
fn an_app_filter_that_does_not_match_the_entry_is_not_found() {
    let world = FakeWorld::new(&log(), vec![reading(&[spec("A", "one")])]);
    let any = at_one("A");
    let error = locate(&world, &wanted(Some("zzz"), &any), deadline())
        .err()
        .expect("none");
    assert_eq!(error.code, ErrorCode::NotificationNotFound);
}

#[test]
fn the_same_words_on_another_object_are_not_the_entry_that_was_read_first() {
    let first = FakeWorld::new(&log(), vec![reading(&[spec("A", "one")])]);
    let any = at_one("A");
    let located = locate(&first, &wanted(None, &any), deadline()).expect("located");
    let rebuilt = FakeWorld::new(
        &log(),
        vec![reading(&[spec("A", "one").object("A:one#new")])],
    );
    let error = reconfirm(&rebuilt, &wanted(None, &any), located.entry(), deadline())
        .err()
        .expect("another object");
    assert_eq!(error.code, ErrorCode::NotificationNotFound);
    assert!(
        error
            .suggestion
            .as_deref()
            .is_some_and(|text| text.contains("key")),
        "{:?}",
        error.suggestion
    );
}
