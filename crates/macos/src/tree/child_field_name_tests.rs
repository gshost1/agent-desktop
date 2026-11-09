use super::*;

fn field() -> AXElement {
    AXElement(std::ptr::null_mut())
}

fn read(text_fields: usize) -> ChildLabelRead {
    ChildLabelRead {
        label: None,
        complete: true,
        capped: false,
        text_fields: (0..text_fields).map(|_| field()).collect(),
        secure_fields: 0,
    }
}

#[test]
fn a_nameless_row_tree_item_or_cell_takes_its_sole_text_field() {
    for role in ["row", "treeitem", "cell"] {
        assert!(sole_text_field(role, &read(1)).is_some(), "{role}");
    }
}

#[test]
fn other_roles_never_take_a_text_field_child_as_their_name() {
    for role in ["combobox", "button", "group", "listbox"] {
        assert!(sole_text_field(role, &read(1)).is_none(), "{role}");
    }
}

#[test]
fn an_existing_child_label_keeps_its_precedence() {
    let mut labelled = read(1);
    labelled.label = Some("Today".into());

    assert!(sole_text_field("treeitem", &labelled).is_none());
}

#[test]
fn two_text_fields_name_nothing() {
    assert!(sole_text_field("row", &read(2)).is_none());
    assert!(sole_text_field("row", &read(0)).is_none());
}

#[test]
fn a_secure_field_beside_the_text_field_names_nothing() {
    let mut with_secure = read(1);
    with_secure.secure_fields = 1;

    assert!(sole_text_field("row", &with_secure).is_none());
}

#[test]
fn a_capped_or_incomplete_child_pass_cannot_prove_the_field_is_the_only_one() {
    let mut capped = read(1);
    capped.capped = true;
    let mut incomplete = read(1);
    incomplete.complete = false;

    assert!(sole_text_field("row", &capped).is_none());
    assert!(sole_text_field("row", &incomplete).is_none());
}

#[test]
fn a_secure_subrole_never_yields_a_name() {
    assert_eq!(
        field_name(
            Some("AXSecureTextField"),
            true,
            Some("hunter2".into()),
            true
        ),
        None
    );
    assert_eq!(
        field_name(None, true, Some("notes.txt".into()), true),
        Some("notes.txt".into())
    );
}

#[test]
fn an_unfinished_read_or_blank_value_yields_no_name() {
    assert_eq!(field_name(None, false, Some("a.txt".into()), true), None);
    assert_eq!(field_name(None, true, Some("a.txt".into()), false), None);
    assert_eq!(field_name(None, true, Some("  ".into()), true), None);
    assert_eq!(field_name(None, true, None, true), None);
}

#[test]
fn the_derived_name_is_carried_only_beside_an_absent_accessible_name() {
    let derived = derived_descriptor(&LocatorField::Absent, Some("notes.txt".into()));
    assert_eq!(derived.derived_name.as_deref(), Some("notes.txt"));
    assert_eq!(derived.name_source, Some(NameSource::ChildFieldValue));

    for name in [LocatorField::Known("Row".into()), LocatorField::Unknown] {
        assert_eq!(
            derived_descriptor(&name, Some("notes.txt".into())),
            NodeDescriptor::default()
        );
    }
}
