use std::sync::Mutex;

use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{AdapterError, CommandContext, NotificationFilter, NotificationInfo};

struct ScriptedNotifications {
    responses: Mutex<Vec<Vec<NotificationInfo>>>,
}

impl ScriptedNotifications {
    fn new(in_order: Vec<Vec<NotificationInfo>>) -> Self {
        let mut responses = in_order;
        responses.reverse();
        Self {
            responses: Mutex::new(responses),
        }
    }
}

impl ObservationOps for ScriptedNotifications {}
impl ActionOps for ScriptedNotifications {}
impl InputOps for ScriptedNotifications {}

impl SystemOps for ScriptedNotifications {
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

    let err = wait(&adapter, &CommandContext::default()).unwrap_err();

    let crate::AppError::Adapter(err) = err else {
        panic!("expected an adapter timeout, got {err:?}");
    };
    assert_eq!(err.code, crate::ErrorCode::Timeout, "{err:?}");
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

#[test]
fn a_seeded_baseline_reports_an_entry_already_in_the_first_list() {
    let adapter = ScriptedNotifications::new(vec![vec![mail(1, "Sent", &[])]]);
    let context = CommandContext::default()
        .with_notification_baseline(Some(crate::NotificationBaseline::from_list(&[])));

    let value = wait(&adapter, &context).unwrap();

    assert_eq!(value["notification"]["title"], "Sent", "{value}");
}
