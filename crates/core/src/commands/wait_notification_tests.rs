use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{AdapterError, CommandContext, NotificationFilter, NotificationInfo};

struct ScriptedNotifications {
    responses: Mutex<Vec<Vec<NotificationInfo>>>,
    probes: AtomicUsize,
}

impl ScriptedNotifications {
    fn new(in_order: Vec<Vec<NotificationInfo>>) -> Self {
        let mut responses = in_order;
        responses.reverse();
        Self {
            responses: Mutex::new(responses),
            probes: AtomicUsize::new(0),
        }
    }
}

impl ObservationOps for ScriptedNotifications {}
impl ActionOps for ScriptedNotifications {}
impl InputOps for ScriptedNotifications {}

impl SystemOps for ScriptedNotifications {
    fn check_notification_support(&self, _deadline: crate::Deadline) -> Result<(), AdapterError> {
        self.probes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn list_notifications(
        &self,
        _filter: &NotificationFilter,
        _policy: crate::InteractionPolicy,
        _deadline: crate::Deadline,
        _lease: Option<&crate::InteractionLease>,
    ) -> Result<Vec<NotificationInfo>, AdapterError> {
        self.responses
            .lock()
            .unwrap()
            .pop()
            .ok_or_else(|| AdapterError::timeout("notification center unavailable"))
    }
}

fn mail(index: usize, title: &str, actions: &[&str]) -> NotificationInfo {
    NotificationInfo {
        index,
        app_name: "Mail".into(),
        title: title.into(),
        body: Some("body".into()),
        actions: actions.iter().map(|action| (*action).into()).collect(),
    }
}

fn wait(
    adapter: &ScriptedNotifications,
    context: &CommandContext,
) -> Result<serde_json::Value, crate::AppError> {
    super::wait_for_notification(None, None, 600, adapter, context)
}

#[test]
fn dismissed_then_identical_notification_is_matched() {
    let adapter = ScriptedNotifications::new(vec![
        vec![mail(1, "Build failed", &[])],
        vec![],
        vec![],
        vec![mail(1, "Build failed", &[])],
    ]);

    let value = wait(&adapter, &CommandContext::default()).unwrap();

    assert_eq!(value["matched"], true, "{value}");
    assert_eq!(value["notification"]["title"], "Build failed");
}

#[test]
fn a_changed_actions_list_alone_is_not_a_new_notification() {
    let adapter = ScriptedNotifications::new(vec![
        vec![mail(1, "Invite", &[])],
        vec![mail(1, "Invite", &["Accept", "Decline"])],
    ]);

    assert_timeout(wait(&adapter, &CommandContext::default()));
}

#[test]
fn a_reindexed_existing_notification_is_not_new() {
    let adapter = ScriptedNotifications::new(vec![
        vec![mail(1, "Old", &[])],
        vec![mail(1, "Fresh", &[]), mail(2, "Old", &[])],
    ]);

    let value = wait(&adapter, &CommandContext::default()).unwrap();

    assert_eq!(value["notification"]["title"], "Fresh", "{value}");
}

#[test]
fn a_second_identical_entry_is_new() {
    let adapter = ScriptedNotifications::new(vec![
        vec![mail(1, "Same", &[])],
        vec![mail(1, "Same", &[]), mail(2, "Same", &[])],
    ]);

    let value = wait(&adapter, &CommandContext::default()).unwrap();

    assert_eq!(value["notification"]["index"], 2, "{value}");
}

fn assert_timeout(result: Result<serde_json::Value, crate::AppError>) -> crate::AdapterError {
    let err = result.unwrap_err();
    let crate::AppError::Adapter(err) = err else {
        panic!("expected an adapter timeout, got {err:?}");
    };
    assert_eq!(err.code, crate::ErrorCode::Timeout, "{err:?}");
    err
}

#[test]
fn one_empty_read_between_full_reads_is_not_a_new_notification() {
    let adapter = ScriptedNotifications::new(vec![
        vec![mail(1, "Build failed", &[])],
        vec![],
        vec![mail(1, "Build failed", &[])],
    ]);

    assert_timeout(wait(&adapter, &CommandContext::default()));
}

#[test]
fn a_captured_seed_reports_an_entry_already_in_the_first_list_without_a_second_probe() {
    let adapter = ScriptedNotifications::new(vec![vec![mail(1, "Sent", &[])]]);
    let seed = crate::NotificationSeed::Captured(crate::NotificationBaseline::from_list(&[]));
    let context = CommandContext::default().with_notification_seed(Some(seed));

    let value = wait(&adapter, &context).unwrap();

    assert_eq!(value["notification"]["title"], "Sent", "{value}");
    assert!(value.get("pre_action_seed").is_none(), "{value}");
    assert_eq!(adapter.probes.load(Ordering::SeqCst), 0);
}

#[test]
fn an_unavailable_seed_falls_back_to_the_first_list_and_says_so() {
    let adapter = ScriptedNotifications::new(vec![
        vec![mail(1, "Old", &[])],
        vec![mail(1, "Old", &[]), mail(2, "Fresh", &[])],
    ]);
    let context = CommandContext::default()
        .with_notification_seed(Some(crate::NotificationSeed::Unavailable));

    let value = wait(&adapter, &context).unwrap();

    assert_eq!(value["notification"]["title"], "Fresh", "{value}");
    assert_eq!(value["pre_action_seed"], "unavailable", "{value}");
    assert_eq!(adapter.probes.load(Ordering::SeqCst), 1);
}

#[test]
fn a_skipped_seed_is_named_on_a_timeout() {
    let adapter = ScriptedNotifications::new(vec![vec![]]);
    let context = CommandContext::default()
        .with_notification_seed(Some(crate::NotificationSeed::SkippedHeaded));

    let err = assert_timeout(wait(&adapter, &context));

    assert_eq!(err.details.unwrap()["pre_action_seed"], "skipped_headed");
}
