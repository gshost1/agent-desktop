use super::vertical_overflow;

#[test]
fn only_index_snapshots_request_vertical_bar_enabled_evidence() {
    use agent_desktop_core::ObservationMode;
    let index = ObservationMode {
        contents_index: true,
        ..Default::default()
    };
    assert!(super::reads_contents_index(index));
    for mode in [
        ObservationMode::default(),
        ObservationMode {
            identity_only: true,
            ..index
        },
        ObservationMode {
            text_content: true,
            ..index
        },
    ] {
        assert!(!super::reads_contents_index(mode));
    }
}

#[test]
fn presentation_states_request_vertical_bar_without_actions() {
    use crate::tree::properties::node_attribute_names::attribute_mask;
    use crate::tree::properties::node_attribute_status::{
        HORIZONTAL_SCROLLBAR, VERTICAL_SCROLLBAR, attribute_bit,
    };
    let requirements = agent_desktop_core::EvidenceRequirements {
        states: true,
        ..Default::default()
    };
    let mask = attribute_mask(requirements);
    assert_ne!(mask & attribute_bit(VERTICAL_SCROLLBAR), 0);
    assert_eq!(mask & attribute_bit(HORIZONTAL_SCROLLBAR), 0);
    assert_eq!(
        attribute_mask(Default::default()) & attribute_bit(VERTICAL_SCROLLBAR),
        0
    );
}

#[test]
fn action_evidence_still_requests_both_scrollbars() {
    use crate::tree::properties::node_attribute_names::attribute_mask;
    use crate::tree::properties::node_attribute_status::{
        HORIZONTAL_SCROLLBAR, VERTICAL_SCROLLBAR, attribute_bit,
    };
    let mut requirements = agent_desktop_core::EvidenceRequirements::default();
    requirements.ref_evidence.actions = true;
    let mask = attribute_mask(requirements);
    for index in [VERTICAL_SCROLLBAR, HORIZONTAL_SCROLLBAR] {
        assert_ne!(mask & attribute_bit(index), 0);
    }
}

#[test]
fn enabled_vertical_bar_reports_overflow() {
    assert_eq!(vertical_overflow(Some(true), Ok(Some(true))), Some(true));
}

#[test]
fn disabled_vertical_bar_reports_fitting() {
    assert_eq!(vertical_overflow(Some(true), Ok(Some(false))), Some(false));
}

#[test]
fn absent_or_failed_bar_never_reports_fitting_or_overflow() {
    for bar in [Some(false), None] {
        for enabled in [Ok(Some(true)), Ok(Some(false)), Ok(None), Err(-25204)] {
            assert_eq!(vertical_overflow(bar, enabled), None);
        }
    }
}

#[test]
fn missing_malformed_or_failed_enabled_read_stays_unknown() {
    for enabled in [Ok(None), Err(i32::MIN), Err(-25204)] {
        assert_eq!(vertical_overflow(Some(true), enabled), None);
    }
}
