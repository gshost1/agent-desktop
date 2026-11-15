//! Pins the shared resolver-error payload byte-for-byte against the two
//! platform copies these constructors replaced
//! (`crates/windows/src/tree/resolve/resolve_search.rs::identity_unknown_error`,
//! `crates/macos/src/tree/resolve/resolve_errors.rs::identity_unknown`, and both
//! platforms' `mark_deadline_elapsed`, captured before deletion). Renaming or
//! dropping a key here is a behaviour change for both adapters at once.

use super::*;

fn entry_with_role(role: &str) -> RefEntry {
    crate::adapter::minimal_ref_entry(role, None)
}

#[test]
fn identity_unknown_error_pins_the_shared_payload() {
    let entry = entry_with_role("button");
    let error = identity_unknown_error(&entry);

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
    assert_eq!(
        error.message,
        "Strict resolution could not determine candidate identity from the live accessibility evidence"
    );
    assert_eq!(
        error.suggestion.as_deref(),
        Some("Retry after the target application finishes updating its accessibility tree")
    );
    assert_eq!(
        error.details,
        Some(json!({
            "kind": "resolution_identity_unknown",
            "role": "button",
            "complete": false,
            "retryable": true,
        }))
    );
    assert!(error.is_explicitly_retryable());
    assert!(error.permits_retry_by_default());
}

#[test]
fn mark_deadline_elapsed_stamps_an_object_details_payload() {
    let error = AdapterError::new(ErrorCode::AppUnresponsive, "incomplete").with_details(json!({
        "kind": "resolution_identity_unknown",
        "complete": false,
        "retryable": true,
    }));

    let stamped = mark_deadline_elapsed(error);

    assert_eq!(
        stamped.details,
        Some(json!({
            "kind": "resolution_identity_unknown",
            "complete": false,
            "retryable": true,
            "deadline_elapsed": true,
        }))
    );
}

#[test]
fn mark_deadline_elapsed_wraps_a_non_object_details_payload() {
    let error =
        AdapterError::new(ErrorCode::Timeout, "bare").with_details(json!("evidence-string"));

    let stamped = mark_deadline_elapsed(error);

    assert_eq!(
        stamped.details,
        Some(json!({
            "evidence": "evidence-string",
            "deadline_elapsed": true,
        }))
    );
}

#[test]
fn mark_deadline_elapsed_fills_missing_details() {
    let error = AdapterError::new(ErrorCode::Timeout, "bare");

    let stamped = mark_deadline_elapsed(error);

    assert_eq!(stamped.details, Some(json!({ "deadline_elapsed": true })));
}

fn captured(role: &str, verdict: CaptureVerdict, surface: crate::SnapshotSurface) -> RefEntry {
    let mut entry = entry_with_role(role);
    entry.identity.capture = verdict;
    entry.source.source_surface = surface;
    entry
}

fn refusal(role: &str, verdict: CaptureVerdict, surface: crate::SnapshotSurface) -> AdapterError {
    refused_before_reading(&captured(role, verdict, surface)).expect("refused before reading")
}

#[test]
fn a_unique_capture_is_not_refused() {
    let entry = captured(
        "button",
        CaptureVerdict::Unique,
        crate::SnapshotSurface::Window,
    );

    assert!(refused_before_reading(&entry).is_none());
}

#[test]
fn a_twin_control_names_what_the_capture_saw_and_no_older_refmap() {
    let error = refusal(
        "button",
        CaptureVerdict::Ambiguous,
        crate::SnapshotSurface::Window,
    );

    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
    assert_eq!(
        error.details.as_ref().unwrap()["kind"],
        "identity_not_unique"
    );
    let hint = error.suggestion.unwrap();
    assert!(hint.contains("another element in its window"), "{hint}");
    assert!(hint.contains("--headed mouse-click --xy"), "{hint}");
    assert!(!hint.contains("earlier version"), "{hint}");
    assert!(!hint.contains("--surface menu"), "{hint}");
}

#[test]
fn a_twin_menu_title_from_a_window_capture_offers_the_keyboard_for_a_menu_bar_entry() {
    let error = refusal(
        "menu",
        CaptureVerdict::Ambiguous,
        crate::SnapshotSurface::Window,
    );

    let hint = error.suggestion.unwrap();
    assert!(hint.contains("menu-bar entry itself"), "{hint}");
    assert!(hint.contains("keyboard"), "{hint}");
}

#[test]
fn every_twin_text_warns_that_a_ref_from_before_two_seven_may_not_be_a_twin() {
    for (role, surface) in [
        ("button", crate::SnapshotSurface::Window),
        ("menuitem", crate::SnapshotSurface::Window),
        ("menu", crate::SnapshotSurface::Window),
        ("menuitem", crate::SnapshotSurface::Menu),
    ] {
        let hint = refusal(role, CaptureVerdict::Ambiguous, surface)
            .suggestion
            .unwrap();
        assert!(
            hint.contains("before 2.7 may not be a twin"),
            "{role}: {hint}"
        );
        assert!(
            hint.contains("role, name, description and native id"),
            "{role}: {hint}"
        );
    }
}

#[test]
fn a_twin_menu_entry_from_a_window_capture_points_at_the_menu_surface() {
    for role in ["menuitem", "menu"] {
        for surface in [
            crate::SnapshotSurface::Window,
            crate::SnapshotSurface::Menubar,
        ] {
            let error = refusal(role, CaptureVerdict::Ambiguous, surface);

            assert_eq!(error.code, ErrorCode::AmbiguousTarget);
            let hint = error.suggestion.unwrap();
            assert!(
                hint.contains("snapshot --surface menu --app <app> -i"),
                "{role} {surface:?}: {hint}"
            );
            assert!(!hint.contains("earlier version"), "{hint}");
        }
    }
}

#[test]
fn a_twin_inside_the_open_menu_does_not_send_the_caller_back_to_the_menu_surface() {
    let error = refusal(
        "menuitem",
        CaptureVerdict::Ambiguous,
        crate::SnapshotSurface::Menu,
    );

    let hint = error.suggestion.unwrap();
    assert!(hint.contains("open menu"), "{hint}");
    assert!(hint.contains("arrow keys"), "{hint}");
    assert!(!hint.contains("--surface menu"), "{hint}");
}

#[test]
fn an_incomplete_capture_asks_for_a_larger_budget_not_a_twin_workaround() {
    let error = refusal(
        "menuitem",
        CaptureVerdict::Incomplete,
        crate::SnapshotSurface::Window,
    );

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(
        error.details.as_ref().unwrap()["reason"],
        "capture_incomplete"
    );
    let hint = error.suggestion.unwrap();
    assert!(hint.contains("--timeout-ms"), "{hint}");
    assert!(!hint.contains("twin"), "{hint}");
}

#[test]
fn an_unknown_capture_says_identity_cannot_be_read() {
    let error = refusal(
        "button",
        CaptureVerdict::Unknown,
        crate::SnapshotSurface::Window,
    );

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(error.details.as_ref().unwrap()["reason"], "capture_unknown");
    assert!(
        error
            .suggestion
            .unwrap()
            .contains("cannot be read reliably")
    );
}
