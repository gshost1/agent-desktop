use super::*;
use agent_desktop_core::{ErrorCode, SnapshotSurface};

fn focused_entry(source_window_id: Option<&str>) -> RefEntry {
    let mut entry: RefEntry = serde_json::from_value(serde_json::json!({
        "pid": 1,
        "role": "button",
        "path": [],
        "states": [],
        "available_actions": [],
    }))
    .unwrap();
    entry.source.source_surface = SnapshotSurface::Focused;
    entry.source.source_window_id = source_window_id.map(String::from);
    entry
}

fn never_called(_: Option<&'static str>) -> Result<Option<&'static str>, AdapterError> {
    panic!("a focused ref must not re-root in a window it was not captured from")
}

fn window_root(window: Option<&'static str>) -> Result<Option<&'static str>, AdapterError> {
    Ok(window.map(|_| "focused root"))
}

#[test]
fn focused_ref_resolves_in_its_recorded_window_while_it_stays_focused() {
    let entry = focused_entry(Some("w-11"));
    let root = focused_root_for_entry(&entry, Some(("window 11", Some(11))), window_root);
    assert_eq!(root.unwrap(), Some("focused root"));
}

#[test]
fn focused_ref_refuses_when_focus_moved_to_another_window() {
    let entry = focused_entry(Some("w-11"));
    let error =
        focused_root_for_entry(&entry, Some(("window 12", Some(12))), never_called).unwrap_err();
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
fn focused_ref_refuses_when_the_focused_window_number_is_unreadable_or_absent() {
    let entry = focused_entry(Some("w-11"));
    for focused in [Some(("window", None)), None] {
        let error = focused_root_for_entry(&entry, focused, never_called).unwrap_err();
        assert_eq!(error.code, ErrorCode::StaleRef);
        let details = error.details.unwrap();
        assert_eq!(details["kind"], "focused_window_changed");
        assert!(details.get("focused_window_id").is_none());
    }
}

#[test]
fn focused_ref_with_a_malformed_recorded_window_is_refused() {
    let entry = focused_entry(Some("w-abc"));
    let error =
        focused_root_for_entry(&entry, Some(("window", Some(11))), never_called).unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleRef);
}

#[test]
fn focused_ref_without_a_recorded_window_keeps_the_live_focused_surface() {
    let entry = focused_entry(None);
    let root = focused_root_for_entry(&entry, Some(("window 12", Some(12))), window_root);
    assert_eq!(root.unwrap(), Some("focused root"));
    let root = focused_root_for_entry(&entry, None, window_root);
    assert_eq!(root.unwrap(), None);
}
