use super::edits_through_focus;

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
