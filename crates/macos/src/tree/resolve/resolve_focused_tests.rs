use super::*;
use agent_desktop_core::ErrorCode;

fn never_called(_: Option<&'static str>) -> Result<Option<&'static str>, AdapterError> {
    panic!("a focused surface must not be read in a window it was not captured from")
}

fn window_root(window: Option<&'static str>) -> Result<Option<&'static str>, AdapterError> {
    Ok(window.map(|_| "focused root"))
}

#[test]
fn focused_surface_resolves_in_its_source_window_while_it_stays_focused() {
    let root = focused_root_in_source_window("w-11", Some(("window 11", Some(11))), window_root);
    assert_eq!(root.unwrap(), Some("focused root"));
}

#[test]
fn focused_surface_is_refused_when_focus_moved_to_another_window() {
    let error = focused_root_in_source_window("w-11", Some(("window 12", Some(12))), never_called)
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleRef);
    let details = error.details.as_ref().unwrap();
    assert_eq!(details["kind"], "focused_window_changed");
    assert_eq!(details["source_window_id"], "w-11");
    assert_eq!(details["focused_window_id"], "w-12");
    assert_eq!(details["complete"], true);
    assert!(!error.permits_retry_by_default());
    assert_eq!(
        error.disposition,
        agent_desktop_core::DeliverySemantics::not_delivered()
    );
}

#[test]
fn focused_surface_is_refused_when_the_focused_window_number_is_unreadable_or_absent() {
    for focused in [Some(("window", None)), None] {
        let error = focused_root_in_source_window("w-11", focused, never_called).unwrap_err();
        assert_eq!(error.code, ErrorCode::StaleRef);
        let details = error.details.unwrap();
        assert_eq!(details["kind"], "focused_window_changed");
        assert!(details.get("focused_window_id").is_none());
    }
}

#[test]
fn a_malformed_source_window_id_is_refused_as_an_invalid_identity() {
    for id in ["w-abc", "w-0", "11"] {
        let error = focused_root_in_source_window(id, Some(("window", Some(11))), never_called)
            .unwrap_err();
        let details = error.details.unwrap();
        assert_eq!(details["kind"], "source_window_identity_invalid");
        assert_eq!(details["retryable"], false);
    }
}
