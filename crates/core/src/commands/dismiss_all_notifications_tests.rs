use super::{DismissAllNotificationsArgs, execute};
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{
    AdapterError, DismissAllNotificationsRequest, InteractionLease, ListRestore, NotificationInfo,
    NotificationOutcome,
};

struct Center {
    restore: Option<ListRestore>,
}

impl ObservationOps for Center {}
impl ActionOps for Center {}
impl InputOps for Center {}

impl SystemOps for Center {
    crate::adapter::guarded_interaction_lease!();

    fn dismiss_all_notifications_reported(
        &self,
        _request: DismissAllNotificationsRequest<'_>,
        _lease: &InteractionLease,
    ) -> Result<NotificationOutcome<(Vec<NotificationInfo>, Vec<String>)>, AdapterError> {
        Ok(NotificationOutcome {
            value: (Vec::new(), vec!["#1: refused".into()]),
            list_restore: self.restore,
        })
    }
}

fn headed() -> crate::context::CommandContext {
    crate::context::CommandContext::default().with_headed(true)
}

#[test]
fn a_sweep_whose_cleanup_failed_says_the_restore_is_unverified() {
    let center = Center {
        restore: Some(ListRestore::Unverified),
    };

    let data = execute(
        DismissAllNotificationsArgs { app: None },
        &center,
        &headed(),
    )
    .unwrap();

    assert_eq!(data["list_restore"], "unverified");
    assert_eq!(data["failed_count"], 1);
}

#[test]
fn a_clean_sweep_carries_no_restore_field() {
    let center = Center { restore: None };

    let data = execute(
        DismissAllNotificationsArgs { app: None },
        &center,
        &headed(),
    )
    .unwrap();

    assert!(data.get("list_restore").is_none(), "{data}");
    assert_eq!(data["dismissed_count"], 0);
}
