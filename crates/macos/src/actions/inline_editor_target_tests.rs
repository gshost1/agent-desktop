use agent_desktop_core::actions::capability;

#[test]
fn a_field_under_an_editor_gains_type_and_set_value_once() {
    let mut actions = vec![capability::SET_VALUE.to_string()];
    super::add_editor_capabilities(&mut actions);
    super::add_editor_capabilities(&mut actions);

    assert_eq!(
        actions,
        vec![
            capability::SET_VALUE.to_string(),
            capability::TYPE_TEXT.to_string()
        ]
    );
}

#[test]
fn only_text_fields_missing_a_text_capability_pay_for_the_editor_probe() {
    let set_value = vec![capability::SET_VALUE.to_string()];
    let both = vec![
        capability::SET_VALUE.to_string(),
        capability::TYPE_TEXT.to_string(),
    ];

    assert!(super::needs_editor_probe("textfield", &set_value));
    assert!(super::needs_editor_probe("combobox", &[]));
    assert!(!super::needs_editor_probe("textfield", &both));
    assert!(!super::needs_editor_probe("button", &set_value));
    assert!(!super::needs_editor_probe("row", &[]));
}
