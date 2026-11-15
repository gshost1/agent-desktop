use super::{
    NcSession, NcSessionOps, closed_center_policy_error, merge_session_result, nc_pid_from_output,
};
use agent_desktop_core::{
    ActionResult, AdapterError, DeliverySemantics, ErrorCode, NotificationInfo, ProcessIdentity,
};

#[test]
fn nc_pid_preserves_probe_errors() {
    let error = nc_pid_from_output(Err(AdapterError::timeout("pid probe timed out")))
        .expect_err("timeout must not become process-not-found");

    assert_eq!(error.code, ErrorCode::Timeout);
}

#[test]
fn closed_notification_center_is_policy_denied_headlessly() {
    let error = closed_center_policy_error(agent_desktop_core::InteractionPolicy::headless());

    assert_eq!(error.code, ErrorCode::PolicyDenied);
    assert!(error.message.contains("headless"));
    assert!(
        error
            .suggestion
            .as_deref()
            .is_some_and(|value| value.contains("--headed"))
    );
}

#[test]
fn cleanup_retries_only_the_failed_step_with_a_fresh_budget() {
    let mut session = NcSession {
        pid: 7,
        close_pending: true,
        previous_app: Some(ProcessIdentity::new(9_u32, "instance")),
        cleanup_on_drop: false,
    };
    let mut close_attempts = 0;
    let mut restore_attempts = 0;

    let error = session
        .cleanup_with(
            |deadline| {
                assert!(!deadline.is_expired());
                close_attempts += 1;
                Err(AdapterError::timeout("close failed"))
            },
            |_, deadline| {
                assert!(!deadline.is_expired());
                restore_attempts += 1;
                Ok(())
            },
        )
        .expect_err("close failure must be reported");

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(close_attempts, 1);
    assert_eq!(restore_attempts, 1);
    assert!(session.close_pending);
    assert!(session.previous_app.is_none());

    session
        .cleanup_with(
            |deadline| {
                assert!(!deadline.is_expired());
                close_attempts += 1;
                Ok(())
            },
            |_, _| panic!("successful restoration must not repeat"),
        )
        .unwrap();
    assert_eq!(close_attempts, 2);
    assert!(!session.close_pending);
}

#[test]
fn operation_failure_wins_when_cleanup_also_fails() {
    let operation = AdapterError::new(ErrorCode::ElementNotFound, "operation failed");
    let cleanup = AdapterError::timeout("cleanup failed");

    let error = merge_session_result::<NotificationInfo>(Err(operation), Err(cleanup)).unwrap_err();

    assert_eq!(error.code, ErrorCode::ElementNotFound);
}

fn restore_failure() -> Result<(), AdapterError> {
    Err(AdapterError::new(ErrorCode::PermDenied, "restore failed")
        .with_disposition(DeliverySemantics::not_delivered()))
}

fn notification(title: &str) -> NotificationInfo {
    NotificationInfo {
        index: 1,
        app_name: "Fixture".into(),
        title: title.into(),
        body: None,
        actions: Vec::new(),
    }
}

#[test]
fn a_delivered_action_keeps_its_result_when_cleanup_fails() {
    let pressed = ActionResult::delivered_unverified("Open");

    let kept = merge_session_result(Ok(pressed), restore_failure())
        .expect("a delivered action must not become a retry-safe error");

    assert_eq!(
        kept.disposition(),
        DeliverySemantics::delivered_unverified()
    );
    assert_eq!(kept.details.unwrap()["list_restore"], "unverified");
}

#[test]
fn a_delivered_dismissal_keeps_the_dismissed_entry_when_cleanup_fails() {
    let kept = merge_session_result(Ok(notification("a")), restore_failure())
        .expect("a delivered dismissal must not become a retry-safe error");

    assert_eq!(kept.title, "a");
}

#[test]
fn a_dismiss_all_keeps_its_counts_when_cleanup_fails() {
    let swept = (vec![notification("a")], vec!["#2: failed".to_owned()]);

    let (dismissed, failures) = merge_session_result(Ok(swept), restore_failure())
        .expect("a delivered sweep must not become a retry-safe error");

    assert_eq!(dismissed.len(), 1);
    assert_eq!(failures.len(), 1);
}

#[test]
fn a_read_whose_cleanup_fails_reports_the_cleanup_failure() {
    let error = merge_session_result(Ok(vec![notification("a")]), restore_failure()).unwrap_err();

    assert_eq!(error.code, ErrorCode::PermDenied);
}

#[test]
fn an_operation_error_names_the_unverified_restore_when_cleanup_also_fails() {
    let operation = AdapterError::new(ErrorCode::ActionFailed, "operation failed")
        .with_disposition(DeliverySemantics::delivered_unverified());

    let error =
        merge_session_result::<NotificationInfo>(Err(operation), restore_failure()).unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert_eq!(error.details.unwrap()["list_restore"], "unverified");
}

#[test]
fn explicit_close_returns_success_when_bounded_retry_recovers() {
    let mut session = NcSession {
        pid: 7,
        close_pending: true,
        previous_app: None,
        cleanup_on_drop: true,
    };
    let attempts = std::cell::Cell::new(0);

    let result = session.close_with(
        |_| {
            attempts.set(attempts.get() + 1);
            if attempts.get() == 1 {
                Err(AdapterError::timeout("transient close failure"))
            } else {
                Ok(())
            }
        },
        |_, _| Ok(()),
    );

    assert!(result.is_ok());
    assert_eq!(attempts.get(), 2);
    assert!(!session.cleanup_on_drop);
}

#[test]
fn explicit_close_returns_final_error_without_arming_drop_retry() {
    let mut session = NcSession {
        pid: 7,
        close_pending: true,
        previous_app: None,
        cleanup_on_drop: true,
    };
    let attempts = std::cell::Cell::new(0);

    let error = session
        .close_with(
            |_| {
                attempts.set(attempts.get() + 1);
                Err(AdapterError::timeout(format!(
                    "close attempt {} failed",
                    attempts.get()
                )))
            },
            |_, _| Ok(()),
        )
        .unwrap_err();

    assert_eq!(attempts.get(), 2);
    assert!(error.message.contains("attempt 2"));
    assert!(!session.cleanup_on_drop);
}

#[test]
fn partial_open_failure_closes_center_and_restores_previous_app() {
    let previous = ProcessIdentity::new(9_u32, "instance");
    let mut close_attempts = 0;
    let mut restore_attempts = 0;

    let result = NcSession::open_with(
        Some(previous.clone()),
        agent_desktop_core::Deadline::after(0).unwrap(),
        NcSessionOps {
            open: |_| Ok(()),
            wait_until_ready: |_| Err(AdapterError::timeout("readiness failed")),
            close: |deadline| {
                assert!(!deadline.is_expired());
                close_attempts += 1;
                Ok(())
            },
            reactivate: |app, deadline| {
                assert_eq!(app, &previous);
                assert!(!deadline.is_expired());
                restore_attempts += 1;
                Ok(())
            },
        },
    );
    let error = match result {
        Ok(_) => panic!("readiness failure must be preserved"),
        Err(error) => error,
    };

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(close_attempts, 1);
    assert_eq!(restore_attempts, 1);
}

#[test]
fn close_delay_does_not_consume_focus_restoration_budget() {
    let mut session = NcSession {
        pid: 7,
        close_pending: true,
        previous_app: Some(ProcessIdentity::new(9_u32, "instance")),
        cleanup_on_drop: false,
    };
    let mut restore_attempted = false;
    let close_deadline = std::cell::Cell::new(None);

    session
        .cleanup_with(
            |deadline| {
                close_deadline.set(Some(deadline));
                std::thread::sleep(std::time::Duration::from_millis(200));
                Err(AdapterError::timeout("close timed out"))
            },
            |_, deadline| {
                restore_attempted = true;
                assert_ne!(Some(deadline), close_deadline.get());
                assert!(!deadline.is_expired());
                Ok(())
            },
        )
        .expect_err("close failure must remain visible");

    assert!(restore_attempted);
    assert!(session.previous_app.is_none());
    session.close_pending = false;
}
