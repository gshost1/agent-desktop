use super::*;

fn labels(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

#[test]
fn a_row_matches_any_of_its_labels_ignoring_case_and_padding() {
    let row = labels(&["", "Display P3", "RGB"]);
    assert!(labels_match(&row, "display p3"));
    assert!(labels_match(&row, " RGB "));
}

#[test]
fn a_row_never_matches_a_partial_or_empty_value() {
    let row = labels(&["Display P3"]);
    assert!(!labels_match(&row, "Display"));
    assert!(!labels_match(&row, "  "));
    assert!(!labels_match(&[], "Display P3"));
}

#[test]
fn only_a_read_back_selection_reports_verified() {
    assert!(verified_from_outcome(DeliveryOutcome::DeliveredVerified).unwrap());
    assert!(verified_from_outcome(DeliveryOutcome::SatisfiedNoDelivery).unwrap());
}

#[test]
fn a_row_without_a_selection_write_is_refused_before_delivery() {
    let error = verified_from_outcome(DeliveryOutcome::NotDelivered).unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn a_delivered_write_without_readback_is_not_retry_safe() {
    for outcome in [
        DeliveryOutcome::DeliveredUnverified,
        DeliveryOutcome::DeliveredWithoutEffect,
    ] {
        let error = verified_from_outcome(outcome).unwrap_err();
        assert_eq!(error.code, ErrorCode::ActionFailed);
        assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    }
}

#[test]
fn an_unmatched_value_is_refused_without_delivery() {
    let error = no_matching_label("Missing");
    assert_eq!(error.code, ErrorCode::ElementNotFound);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}
