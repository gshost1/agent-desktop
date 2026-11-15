use super::EditCommit;

#[test]
fn only_a_plain_text_field_waits_for_editing_to_end() {
    assert_eq!(
        EditCommit::from_roles(Some("AXTextField"), None),
        EditCommit::OnEndEditing
    );
    assert!(
        EditCommit::from_roles(Some("AXTextField"), None)
            .hint()
            .is_some_and(|hint| hint.contains("`focus`"))
    );
}

#[test]
fn a_search_field_secure_field_or_popup_is_settled_by_the_read_back() {
    for (role, subrole) in [
        ("AXTextField", Some("AXSearchField")),
        ("AXTextField", Some("AXSecureTextField")),
        ("AXSearchField", None),
        ("AXSecureTextField", None),
        ("AXPopUpButton", None),
        ("AXComboBox", None),
        ("AXSlider", None),
    ] {
        let commit = EditCommit::from_roles(Some(role), subrole);
        assert_eq!(commit, EditCommit::Immediate, "{role} {subrole:?}");
        assert_eq!(commit.hint(), None, "{role} {subrole:?}");
    }
}

#[test]
fn a_text_view_is_unobserved_and_settled_only_by_the_saved_result() {
    let commit = EditCommit::from_roles(Some("AXTextArea"), None);
    assert_eq!(commit, EditCommit::TextView);
    let hint = commit.hint().expect("text view hint");
    assert!(hint.contains("check the saved result"), "{hint}");
    assert_ne!(Some(hint), EditCommit::OnEndEditing.hint());
}

#[test]
fn an_unread_role_fails_closed_to_an_unobserved_commit() {
    assert!(EditCommit::Unread.hint().is_some());
    assert_eq!(EditCommit::from_roles(None, None), EditCommit::Immediate);
}
