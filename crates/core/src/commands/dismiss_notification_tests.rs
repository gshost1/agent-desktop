use super::{DismissNotificationArgs, execute};
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{
    AdapterError, DismissNotificationRequest, InteractionLease, ListRestore, NotificationInfo,
    NotificationOutcome,
};

fn dismissed() -> NotificationInfo {
    NotificationInfo {
        index: 1,
        app_name: "CI".into(),
        title: "Build failed".into(),
        body: None,
        actions: vec![],
    }
}

struct Center {
    restore: Option<ListRestore>,
}

impl ObservationOps for Center {}
impl ActionOps for Center {}
impl InputOps for Center {}

impl SystemOps for Center {
    crate::adapter::guarded_interaction_lease!();

    fn dismiss_notification(
        &self,
        _request: DismissNotificationRequest<'_>,
        _lease: &InteractionLease,
    ) -> Result<NotificationInfo, AdapterError> {
        Ok(dismissed())
    }

    fn dismiss_notification_reported(
        &self,
        _request: DismissNotificationRequest<'_>,
        _lease: &InteractionLease,
    ) -> Result<NotificationOutcome<NotificationInfo>, AdapterError> {
        Ok(NotificationOutcome {
            value: dismissed(),
            list_restore: self.restore,
        })
    }
}

struct PlainCenter;

impl ObservationOps for PlainCenter {}
impl ActionOps for PlainCenter {}
impl InputOps for PlainCenter {}

impl SystemOps for PlainCenter {
    crate::adapter::guarded_interaction_lease!();

    fn dismiss_notification(
        &self,
        _request: DismissNotificationRequest<'_>,
        _lease: &InteractionLease,
    ) -> Result<NotificationInfo, AdapterError> {
        Ok(dismissed())
    }
}

fn args() -> DismissNotificationArgs {
    DismissNotificationArgs {
        index: Some(1),
        key: None,
        app: None,
        expected_app: Some("CI".into()),
        expected_title: None,
    }
}

fn headed() -> crate::context::CommandContext {
    crate::context::CommandContext::default().with_headed(true)
}

#[test]
fn a_dismissal_whose_cleanup_failed_says_the_restore_is_unverified() {
    let center = Center {
        restore: Some(ListRestore::Unverified),
    };

    let data = execute(args(), &center, &headed()).expect("dismissed");

    assert_eq!(data["dismissed"]["title"], "Build failed");
    assert_eq!(data["list_restore"], "unverified");
}

#[test]
fn a_clean_dismissal_carries_no_restore_field() {
    let data = execute(args(), &Center { restore: None }, &headed()).expect("dismissed");

    assert!(data.get("list_restore").is_none(), "{data}");
}

#[test]
fn an_adapter_that_reports_nothing_about_its_cleanup_keeps_the_plain_shape() {
    let data = execute(args(), &PlainCenter, &headed()).expect("dismissed");

    assert_eq!(data["dismissed"]["index"], 1);
    assert!(data.get("list_restore").is_none(), "{data}");
}
