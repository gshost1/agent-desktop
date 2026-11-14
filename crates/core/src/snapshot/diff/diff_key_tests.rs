use super::*;
use crate::adapter::minimal_accessibility_node as node;
use crate::{IdentifierKind, NameSource};

#[test]
fn value_and_states_are_not_identity() {
    let mut before = node("textfield", "Search");
    let mut after = before.clone();
    before.identity.value = Some("ab".into());
    after.identity.value = Some("abc".into());
    after.presentation.states = vec!["focused".into()];
    assert_eq!(IdentityKey::of(&before), IdentityKey::of(&after));
}

#[test]
fn a_stable_name_is_identity() {
    assert_ne!(
        IdentityKey::of(&node("button", "Save")),
        IdentityKey::of(&node("button", "Save As"))
    );
}

#[test]
fn a_name_that_mirrors_a_mutable_value_is_not_identity() {
    let mut before = node("textfield", "ab");
    before.identity.value = Some("ab".into());
    let mut after = node("textfield", "abc");
    after.identity.value = Some("abc".into());
    assert_eq!(IdentityKey::of(&before), IdentityKey::of(&after));
}

#[test]
fn a_derived_name_is_not_identity() {
    let mut before = node("row", "first.txt");
    before.presentation.descriptors.name_source = Some(NameSource::ChildFieldValue);
    let mut after = node("row", "renamed.txt");
    after.presentation.descriptors.name_source = Some(NameSource::ChildFieldValue);
    assert_eq!(IdentityKey::of(&before), IdentityKey::of(&after));
}

#[test]
fn native_id_and_description_are_identity_and_blank_ones_are_not() {
    let mut first = node("button", "OK");
    first.identity.native_id = Some(crate::ElementIdentifier {
        kind: IdentifierKind::AutomationId,
        value: "ok-1".into(),
    });
    let mut second = first.clone();
    second.identity.native_id = Some(crate::ElementIdentifier {
        kind: IdentifierKind::AutomationId,
        value: "ok-2".into(),
    });
    assert_ne!(IdentityKey::of(&first), IdentityKey::of(&second));

    let mut described = node("button", "OK");
    described.identity.description = Some("Confirm".into());
    assert_ne!(IdentityKey::of(&described), IdentityKey::of(&node("button", "OK")));

    let mut blank = node("button", "OK");
    blank.identity.description = Some("  ".into());
    blank.identity.native_id = Some(crate::ElementIdentifier {
        kind: IdentifierKind::AutomationId,
        value: " ".into(),
    });
    assert_eq!(IdentityKey::of(&blank), IdentityKey::of(&node("button", "OK")));
}

#[test]
fn the_key_records_exactly_what_the_ref_capture_predicate_records() {
    let mut field = node("textfield", "Name");
    field.identity.value = Some("George".into());
    field.identity.description = Some("Your name".into());
    let mut mirrored = node("textfield", "draft");
    mirrored.identity.value = Some("draft".into());
    let source = crate::refs::ref_alloc_source::RefAllocSource {
        pid: crate::ProcessId::new(1),
        app: Some("App"),
        process_instance: None,
        window_id: Some("w-1"),
        window_title: Some("Doc"),
        window_bounds_hash: None,
        surface: crate::SnapshotSurface::Window,
    };
    for node in [field, mirrored, node("button", "Save")] {
        let entry = crate::refs::ref_alloc::ref_entry_from_node(&node, &source, None, &[]);
        let predicate = crate::refs::ref_full_predicate::FullPredicate::of_entry(&entry);
        let key = IdentityKey::of(&node);
        assert_eq!(key.role, predicate.role);
        assert_eq!(key.name, predicate.name);
        assert_eq!(key.description, predicate.description);
        assert_eq!(key.native_id, predicate.native_id);
    }
}
