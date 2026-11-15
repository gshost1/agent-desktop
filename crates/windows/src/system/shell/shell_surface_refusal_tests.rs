use agent_desktop_core::{DeliverySemantics, ErrorCode, SnapshotSurface};

use super::{after_send, timeout_error};
use crate::system::shell::shell_surface::row_for;

fn action_center() -> &'static crate::system::shell::shell_surface::SurfaceKindRow {
    row_for(SnapshotSurface::ActionCenter).expect("the action center has a row")
}

#[test]
fn a_timeout_after_the_raise_was_sent_is_not_safe_to_repeat() {
    let error = timeout_error(
        action_center(),
        "did not open within the deadline",
        "shell_surface_not_opened",
        true,
    );

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert!(!error.is_explicitly_retryable());
    assert_eq!(
        error.details.expect("details")["kind"],
        "shell_surface_not_opened"
    );
    let suggestion = error.suggestion.expect("a suggestion");
    assert!(
        suggestion.contains("before you open or close it again"),
        "{suggestion}"
    );
}

#[test]
fn a_timeout_with_nothing_sent_stays_safe_to_repeat() {
    let taskbar = row_for(SnapshotSurface::Taskbar).expect("the taskbar has a row");
    let error = timeout_error(
        taskbar,
        "did not open within the deadline",
        "shell_surface_not_opened",
        false,
    );

    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(error.is_explicitly_retryable());
}

#[test]
fn a_read_that_fails_after_the_input_was_sent_is_not_safe_to_repeat() {
    let unread = agent_desktop_core::AdapterError::new(ErrorCode::Internal, "read failed")
        .with_disposition(DeliverySemantics::not_delivered());

    let error = after_send(unread);

    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let suggestion = error.suggestion.expect("a suggestion");
    assert!(
        suggestion.contains("before you open or close it again"),
        "{suggestion}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn a_declined_foreground_is_safe_before_any_keystroke_and_unsafe_after_a_raise() {
    let before = super::foreground_declined_error(action_center());
    assert_eq!(before.disposition, DeliverySemantics::not_delivered());

    let after = super::declined_after_raise_error(action_center());
    assert_eq!(after.disposition, DeliverySemantics::delivered_unverified());
    assert!(!after.is_explicitly_retryable());
    assert_eq!(
        after.details.expect("details")["kind"],
        "shell_surface_declined_foreground"
    );
    let suggestion = after.suggestion.expect("a suggestion");
    assert!(
        suggestion.contains("before you open or close it again"),
        "{suggestion}"
    );
}
