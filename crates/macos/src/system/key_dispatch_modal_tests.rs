use super::*;
use std::collections::VecDeque;

fn watch() -> ModalWatch {
    ModalWatch {
        kind: "sheet",
        action: "AXConfirm",
        button: ("default_button", false),
        window: None,
    }
}

fn scripted(reads: &[Option<&'static str>]) -> Option<&'static str> {
    let mut reads: VecDeque<Option<&'static str>> = reads.iter().copied().collect();
    settle_with(
        || Ok(reads.pop_front().flatten()),
        Duration::from_millis(120),
        Deadline::standard().unwrap(),
    )
    .unwrap()
}

#[test]
fn a_sheet_window_seen_again_on_the_next_read_is_not_an_effect() {
    assert_eq!(scripted(&[Some("surface_window_closed"), None, None]), None);
    assert_eq!(
        scripted(&[
            None,
            Some("surface_window_closed"),
            None,
            Some("surface_window_closed")
        ]),
        None
    );
}

#[test]
fn a_signal_seen_on_two_consecutive_reads_is_an_effect() {
    assert_eq!(
        scripted(&[
            None,
            Some("surface_window_closed"),
            Some("surface_window_closed")
        ]),
        Some("surface_window_closed")
    );
}

#[test]
fn a_surface_that_never_reacts_settles_within_the_limit() {
    let started = Instant::now();
    assert_eq!(scripted(&[]), None);
    assert!(started.elapsed() < Duration::from_millis(400));
}

#[test]
fn a_surface_whose_window_answers_as_its_host_is_unobservable() {
    assert_eq!(distinct_window(Some(42), Some(42)), None);
    assert_eq!(distinct_window(Some(43), Some(42)), Some(43));
    assert_eq!(distinct_window(Some(43), None), Some(43));
    assert_eq!(distinct_window(None, Some(42)), None);
}

#[test]
fn a_sheet_without_its_own_window_cannot_be_verified() {
    assert!(!watch().observable());
    let watched = ModalWatch {
        window: Some(43),
        ..watch()
    };
    assert!(watched.observable());
}

#[test]
fn an_unobservable_press_reports_an_unverified_delivery() {
    let result = watch().unobservable(vec![ActionStep::succeeded("CGEventPostToPid")]);
    let value = serde_json::to_value(result).unwrap();
    assert_eq!(value["disposition"]["delivery"], "delivered_unverified");
    assert_eq!(value["details"]["modal_key"]["effect"], "unobservable");
}

#[test]
fn a_signal_from_an_unwatched_surface_never_fires_on_its_own() {
    let watched = watch();
    assert_eq!(
        watched.signal(None, Deadline::standard().unwrap()).unwrap(),
        None
    );
}

#[test]
fn sheets_and_dialogs_are_modal_and_standard_windows_are_not() {
    assert_eq!(modal_kind(Some("AXSheet"), None), Some("sheet"));
    assert_eq!(modal_kind(Some("AXWindow"), Some("AXSheet")), Some("sheet"));
    assert_eq!(
        modal_kind(Some("AXWindow"), Some("AXDialog")),
        Some("dialog")
    );
    assert_eq!(
        modal_kind(Some("AXWindow"), Some("AXSystemDialog")),
        Some("dialog")
    );
    assert_eq!(modal_kind(Some("AXWindow"), Some("AXStandardWindow")), None);
    assert_eq!(modal_kind(Some("AXWindow"), Some("AXFloatingWindow")), None);
    assert_eq!(modal_kind(None, None), None);
}

#[test]
fn return_reads_the_default_button_and_escape_the_cancel_button() {
    assert_eq!(
        button_attribute("AXConfirm"),
        ("AXDefaultButton", "default_button")
    );
    assert_eq!(
        button_attribute("AXCancel"),
        ("AXCancelButton", "cancel_button")
    );
}

#[test]
fn an_ignored_confirm_fails_unsafe_to_retry_and_names_the_ref_route() {
    let steps = [
        ActionStep::succeeded("CGEventPostToPid"),
        ActionStep::succeeded("AXConfirm"),
    ];
    let error = watch().ignored(&steps);
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.expect("modal details");
    assert_eq!(details["modal_key"]["effect"], "none_observed");
    assert_eq!(details["modal_key"]["surface"], "sheet");
    assert_eq!(details["modal_key"]["default_button"], false);
    assert_eq!(
        details["modal_key"]["routes"],
        serde_json::json!(["CGEventPostToPid", "AXConfirm"])
    );
    let suggestion = error.suggestion.expect("suggestion");
    assert!(suggestion.contains("Do not repeat the key"), "{suggestion}");
    assert!(suggestion.contains("--headed"), "{suggestion}");
    assert!(suggestion.contains("--surface sheet"), "{suggestion}");
}

#[test]
fn an_ignored_dialog_with_a_default_button_points_at_that_button() {
    let hint = ignored_suggestion("dialog", ("default_button", true));
    assert!(hint.contains("--surface alert"), "{hint}");
    assert!(hint.contains("default button by ref"), "{hint}");
    let hint = ignored_suggestion("sheet", ("cancel_button", true));
    assert!(hint.contains("cancel button by ref"), "{hint}");
}

#[test]
fn an_observed_effect_is_a_verified_delivery_naming_the_signal_and_route() {
    let result = watch().verified(
        vec![
            ActionStep::succeeded("AXConfirm"),
            ActionStep::succeeded("CGEventPostToPid").with_verified(true),
        ],
        "surface_window_closed",
    );
    let value = serde_json::to_value(result).unwrap();
    assert_eq!(value["disposition"]["delivery"], "delivered_verified");
    assert_eq!(value["details"]["modal_key"]["effect"], "observed");
    assert_eq!(
        value["details"]["modal_key"]["signal"],
        "surface_window_closed"
    );
    assert_eq!(
        value["details"]["modal_key"]["observed_after"],
        "CGEventPostToPid"
    );
}
