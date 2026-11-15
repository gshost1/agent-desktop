use agent_desktop_core::{
    ErrorCode, InteractionPolicy, NotificationIdentity, NotificationInfo, NotificationTarget,
};

use super::{
    dismiss_all_failures, is_sole_target, require_foreground_policy, take_target,
    unknown_action_error,
};

fn info(app: &str, title: &str) -> NotificationInfo {
    NotificationInfo {
        index: 1,
        app_name: app.into(),
        title: title.into(),
        body: None,
        actions: Vec::new(),
    }
}

#[test]
fn a_strict_headless_caller_is_policy_denied_before_anything_raises() {
    let error = require_foreground_policy(InteractionPolicy::headless())
        .expect_err("headless policy must refuse");

    assert_eq!(error.code, ErrorCode::PolicyDenied);
}

#[test]
fn a_policy_that_permits_the_foreground_change_proceeds() {
    assert!(require_foreground_policy(InteractionPolicy::focus_fallback()).is_ok());
    assert!(require_foreground_policy(InteractionPolicy::headed()).is_ok());
}

fn entry(index: usize, app: &str, title: &str) -> NotificationInfo {
    NotificationInfo {
        index,
        app_name: app.into(),
        title: title.into(),
        body: None,
        actions: Vec::new(),
    }
}

#[test]
fn a_filtered_dismiss_all_that_removes_its_target_leaves_another_apps_entry_unreported() {
    let captured = vec![entry(1, "App A", "cleared"), entry(2, "App B", "untouched")];
    let current = vec![entry(2, "App B", "untouched")];

    let failures = dismiss_all_failures(&captured, &current, &[]);

    assert_eq!(
        failures.len(),
        1,
        "App A's entry left, so only App B's untouched entry is a survivor"
    );
    assert!(failures[0].contains("#2"));
    assert!(
        !failures.iter().any(|failure| failure.contains("#1")),
        "the entry that actually left must never appear as a failure"
    );
}

#[test]
fn a_recorded_invoke_error_names_its_own_reason_instead_of_the_generic_survivor_message() {
    let captured = vec![entry(1, "App A", "refused"), entry(2, "App B", "untouched")];
    let current = vec![entry(1, "App A", "refused"), entry(2, "App B", "untouched")];
    let invoke_errors = vec![(1, "the dismiss control could not be invoked".to_string())];

    let failures = dismiss_all_failures(&captured, &current, &invoke_errors);

    assert_eq!(failures.len(), 2);
    let for_one = failures
        .iter()
        .find(|failure| failure.contains("#1"))
        .expect("index 1 is reported");
    assert!(
        for_one.contains("the dismiss control could not be invoked"),
        "the invoke's own error must survive into the failure line, not be discarded"
    );
    let for_two = failures
        .iter()
        .find(|failure| failure.contains("#2"))
        .expect("index 2 is reported");
    assert!(
        for_two.contains("still present"),
        "an entry with no recorded invoke error falls back to the generic survivor message"
    );
}

#[test]
fn an_invoke_error_recorded_for_an_entry_that_still_left_is_never_reported() {
    let captured = vec![
        entry(1, "App A", "left anyway"),
        entry(2, "App B", "stayed"),
    ];
    let current = vec![entry(2, "App B", "stayed")];
    let invoke_errors = vec![(1, "transient invoke error".to_string())];

    let failures = dismiss_all_failures(&captured, &current, &invoke_errors);

    assert_eq!(
        failures.len(),
        1,
        "the settle read proves index 1 left despite its invoke error, so it is dismissed, not failed"
    );
    assert!(!failures[0].contains("transient invoke error"));
    assert!(failures[0].contains("#2"));
}

#[test]
fn the_clear_all_substitute_fires_only_on_exactly_one_matching_entry() {
    let target = info("Toaster", "Build finished");
    let foreign = info("Toaster", "Build finished (not the staged entry)");

    assert!(is_sole_target(&target, std::slice::from_ref(&target)));
    assert!(!is_sole_target(&target, std::slice::from_ref(&foreign)));
    assert!(!is_sole_target(&target, &[]));
    assert!(!is_sole_target(&target, &[target.clone(), foreign]));
}

#[test]
fn an_unknown_action_name_is_refused_naming_it_and_nothing_was_delivered() {
    let error = unknown_action_error("agent-desktop-no-such-action", 3);

    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    assert!(error.message.contains("agent-desktop-no-such-action"));
    assert_eq!(
        error.disposition,
        agent_desktop_core::DeliverySemantics::not_delivered()
    );
}

#[test]
fn the_dismiss_reaches_the_windows_override_not_the_trait_default() {
    use agent_desktop_core::{Deadline, DismissNotificationRequest, InteractionLease, SystemOps};

    let adapter = crate::adapter::WindowsAdapter::new();
    let lease = InteractionLease::guarded(Deadline::detached_after(5_000).expect("deadline"), ())
        .expect("lease");
    let identity = NotificationIdentity {
        expected_app: Some("App A".into()),
        expected_title: None,
    };
    let target = NotificationTarget::new(Some(1), None, identity).expect("target");

    let error = SystemOps::dismiss_notification(
        &adapter,
        DismissNotificationRequest {
            target: &target,
            policy: InteractionPolicy::headless(),
        },
        &lease,
    )
    .expect_err("the headless floor refuses before anything is raised");

    assert_eq!(
        error.code,
        ErrorCode::PolicyDenied,
        "the trait default would answer PLATFORM_NOT_SUPPORTED instead"
    );
}

fn shifted_twins() -> (Vec<NotificationInfo>, String, Vec<NotificationInfo>) {
    let before = vec![entry(1, "App A", "Ping"), entry(2, "App A", "Ping")];
    let oldest_key = agent_desktop_core::notification_keys(&before)[1].clone();
    let after = vec![
        entry(1, "App A", "Ping"),
        entry(2, "App A", "Ping"),
        entry(3, "App A", "Ping"),
    ];
    (before, oldest_key, after)
}

#[test]
fn a_key_follows_its_entry_after_an_identical_newcomer_shifts_the_list() {
    let (_, oldest_key, after) = shifted_twins();
    let target = NotificationTarget::new(None, Some(&oldest_key), NotificationIdentity::default())
        .expect("target");

    let taken = take_target(&target, after, |info| info).expect("the keyed entry is taken");

    assert_eq!(
        taken.index, 3,
        "the oldest twin sat at 2 when listed and moved to 3; index 2 now holds a newer twin"
    );
}

#[test]
fn a_key_pinned_to_its_old_index_after_a_shift_is_refused_not_delivered() {
    let (before, oldest_key, after) = shifted_twins();
    let listed_at = before.len();
    let identity = NotificationIdentity {
        expected_app: Some("App A".into()),
        expected_title: Some("Ping".into()),
    };
    let target =
        NotificationTarget::new(Some(listed_at), Some(&oldest_key), identity).expect("target");

    let error = take_target(&target, after, |info| info)
        .expect_err("index 2 now holds a different same-app twin");

    assert_eq!(error.code, ErrorCode::NotificationNotFound);
    assert_eq!(
        error.disposition,
        agent_desktop_core::DeliverySemantics::not_delivered()
    );
}
