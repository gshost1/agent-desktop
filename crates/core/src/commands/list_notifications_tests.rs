use std::sync::Mutex;

use super::keyed;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{
    AdapterError, CommandContext, NotificationFilter, NotificationIdentity, NotificationInfo,
    NotificationTarget,
};

fn info(index: usize, app: &str, body: &str) -> NotificationInfo {
    NotificationInfo {
        index,
        app_name: app.into(),
        title: "New message".into(),
        body: Some(body.into()),
        actions: vec![],
    }
}

fn unfiltered() -> NotificationFilter {
    NotificationFilter::default()
}

fn key_at(entries: &[serde_json::Value], at: usize) -> String {
    entries[at]["key"].as_str().expect("key").to_owned()
}

#[test]
fn every_listed_entry_carries_a_key_and_twins_carry_different_keys() {
    let listed = [info(1, "Slack", "hi"), info(2, "Slack", "hi")];
    let entries = keyed(&listed, &unfiltered()).expect("keyed");
    assert_ne!(key_at(&entries, 0), key_at(&entries, 1));
    assert_eq!(entries[0]["index"], 1);
    assert_eq!(entries[0]["app_name"], "Slack");
}

#[test]
fn a_printed_key_names_its_entry_after_the_list_shifts() {
    let printed = keyed(&[info(1, "Slack", "from Ann")], &unfiltered()).expect("keyed");
    let key = key_at(&printed, 0);
    let target =
        NotificationTarget::new(None, Some(&key), NotificationIdentity::default()).expect("target");
    let now = [info(1, "Slack", "from Bob"), info(2, "Slack", "from Ann")];
    assert_eq!(target.pick(&now).expect("found"), 1);
}

#[test]
fn a_limit_does_not_change_the_key_of_a_shown_twin() {
    let everything = [info(1, "CI", "failed"), info(2, "CI", "failed")];
    let full = keyed(&everything, &unfiltered()).expect("full");
    let limited = NotificationFilter {
        limit: Some(1),
        ..NotificationFilter::default()
    };
    let shown = keyed(&everything, &limited).expect("limited");
    assert_eq!(shown.len(), 1);
    assert_eq!(key_at(&shown, 0), key_at(&full, 0));
}

struct Recording {
    seen: Mutex<Vec<NotificationFilter>>,
}

impl ObservationOps for Recording {}
impl ActionOps for Recording {}
impl InputOps for Recording {}

impl SystemOps for Recording {
    fn list_notifications(
        &self,
        filter: &NotificationFilter,
        _policy: crate::InteractionPolicy,
        _deadline: crate::Deadline,
        _lease: Option<&crate::InteractionLease>,
    ) -> Result<Vec<NotificationInfo>, AdapterError> {
        self.seen.lock().expect("lock").push(filter.clone());
        Ok(vec![info(1, "CI", "failed"), info(2, "Mail", "x")])
    }
}

#[test]
fn the_adapter_reads_the_whole_list_and_core_applies_the_filter() {
    let adapter = Recording {
        seen: Mutex::new(Vec::new()),
    };
    let args = super::ListNotificationsArgs {
        app: Some("mail".into()),
        text: None,
        limit: Some(5),
    };
    let result = super::execute(args, &adapter, &CommandContext::default()).expect("listed");
    let seen = adapter.seen.lock().expect("lock");
    assert!(seen[0].app.is_none() && seen[0].text.is_none() && seen[0].limit.is_none());
    assert_eq!(result["count"], 1);
    assert_eq!(result["notifications"][0]["app_name"], "Mail");
}
