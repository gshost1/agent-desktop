use agent_desktop_core::SystemOps;
use agent_desktop_core::{
    Deadline, DeliverySemantics, DismissAllNotificationsRequest, DismissNotificationRequest,
    ErrorCode, InteractionLease, InteractionPolicy, NotificationActionRequest,
    NotificationIdentity, NotificationTarget,
};

#[test]
fn notification_mutations_fail_before_foreground_access_under_headless_policy() {
    let adapter = agent_desktop_macos::MacOSAdapter::new();
    let deadline = Deadline::standard().unwrap();
    let lease = InteractionLease::guarded(deadline, ()).unwrap();
    let identity = NotificationIdentity {
        expected_app: Some("Messages".into()),
        expected_title: Some("New message".into()),
    };
    let target = NotificationTarget::new(Some(1), None, identity).unwrap();
    let policy = InteractionPolicy::headless();
    let errors = [
        SystemOps::dismiss_notification(
            &adapter,
            DismissNotificationRequest {
                target: &target,
                policy,
            },
            &lease,
        )
        .unwrap_err(),
        SystemOps::dismiss_all_notifications(
            &adapter,
            DismissAllNotificationsRequest {
                app_filter: None,
                policy,
            },
            &lease,
        )
        .unwrap_err(),
        SystemOps::notification_action(
            &adapter,
            NotificationActionRequest {
                target: &target,
                action_name: "Reply",
                policy,
            },
            &lease,
        )
        .unwrap_err(),
    ];

    for error in errors {
        assert_eq!(error.code, ErrorCode::PolicyDenied);
        assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    }
}
