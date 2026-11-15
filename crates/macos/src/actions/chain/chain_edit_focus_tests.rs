use super::{commits_by_confirm, edits_through_focus};

#[test]
fn text_fields_and_combo_boxes_take_focus_before_a_value_write() {
    assert!(edits_through_focus(Some("textfield")));
    assert!(edits_through_focus(Some("combobox")));
}

#[test]
fn other_value_controls_are_written_without_moving_focus() {
    for role in ["slider", "stepper", "checkbox", "incrementor", "button"] {
        assert!(!edits_through_focus(Some(role)), "{role}");
    }
    assert!(!edits_through_focus(None));
}

#[test]
fn single_line_fields_and_combo_boxes_commit_by_confirm() {
    for role in ["AXTextField", "AXComboBox"] {
        assert!(commits_by_confirm(Some(role), None), "{role}");
        assert!(
            commits_by_confirm(Some(role), Some("AXSearchField")),
            "{role}"
        );
    }
}

#[test]
fn text_areas_secure_fields_and_other_controls_are_not_confirmed() {
    assert!(!commits_by_confirm(Some("AXTextArea"), None));
    assert!(!commits_by_confirm(
        Some("AXTextField"),
        Some("AXSecureTextField")
    ));
    assert!(!commits_by_confirm(Some("AXSlider"), None));
    assert!(!commits_by_confirm(None, None));
}
