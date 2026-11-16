use super::EditTarget;

#[test]
fn an_edit_takes_the_write_itself() {
    assert_eq!(
        EditTarget::from_classes(10, "Edit", None),
        EditTarget::Edit(10)
    );
    assert_eq!(
        EditTarget::from_classes(10, "EDIT", Some(11)),
        EditTarget::Edit(10)
    );
}

#[test]
fn a_combo_box_hands_the_write_to_its_edit_child() {
    assert_eq!(
        EditTarget::from_classes(20, "ComboBox", Some(21)),
        EditTarget::Edit(21)
    );
    assert_eq!(
        EditTarget::from_classes(20, "combobox", Some(21)).handle(),
        Some(21)
    );
}

#[test]
fn a_drop_down_list_without_an_edit_keeps_its_value_pattern() {
    assert_eq!(
        EditTarget::from_classes(30, "ComboBox", None),
        EditTarget::NotEdit
    );
    assert_eq!(EditTarget::NotEdit.handle(), None);
}

#[test]
fn other_classes_keep_their_value_pattern_even_with_an_edit_child() {
    for class in ["RICHEDIT50W", "ComboBoxEx32", "Button", "#32770", ""] {
        assert_eq!(
            EditTarget::from_classes(40, class, Some(41)),
            EditTarget::NotEdit,
            "{class}"
        );
    }
}
