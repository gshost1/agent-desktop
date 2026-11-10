use super::*;
use crate::system::display::display_cause::Cause;
use crate::system::display::display_unavailable::Unavailable;
use agent_desktop_core::{DeliveryDisposition, ErrorCode};

#[test]
fn a_session_without_a_display_is_refused_before_the_spawn() {
    let error = require_display(DisplayServer::Unavailable(Unavailable {
        cause: Cause::NotConfigured,
        detail: String::new(),
    }))
    .expect_err("no display");
    assert_eq!(error.code, ErrorCode::PlatformNotSupported);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
    assert_eq!(
        error.details.expect("details")["display_unavailable"],
        "not_configured"
    );
}

#[test]
fn a_session_with_a_display_server_is_allowed() {
    assert!(require_display(DisplayServer::Wayland).is_ok());
    assert!(require_display(DisplayServer::X11(":1".into())).is_ok());
}

#[test]
fn a_failure_after_the_spawn_names_the_spawned_pid_and_keeps_its_details() {
    let error = with_spawned_pid(
        AdapterError::timeout("late").with_details(serde_json::json!({ "phase": "owner" })),
        4242,
    );
    let details = error.details.expect("details");
    assert_eq!(details["spawned_pid"], 4242);
    assert_eq!(details["phase"], "owner");
    let bare = with_spawned_pid(AdapterError::timeout("late"), 7);
    assert_eq!(bare.details.expect("details")["spawned_pid"], 7);
}
