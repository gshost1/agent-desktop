use super::*;
use crate::adapter::minimal_accessibility_node as node;

#[test]
fn fields_are_reported_in_order_and_focus_apart_from_other_states() {
    let mut before = node("textfield", "Name");
    before.identity.value = Some("ab".into());
    before.presentation.states = vec!["required".into()];
    let mut after = before.clone();
    after.identity.value = Some("abc".into());
    after.presentation.hint = Some("Type a name".into());
    after.presentation.states = vec!["focused".into(), "required".into()];
    assert_eq!(changed_fields(&before, &after), ["value", "hint", "focus"]);

    after.presentation.states = vec!["focused".into(), "invalid".into()];
    assert_eq!(
        changed_fields(&before, &after),
        ["value", "hint", "states", "focus"]
    );
}

#[test]
fn state_order_is_not_a_change() {
    let mut before = node("checkbox", "Bold");
    before.presentation.states = vec!["checked".into(), "focused".into()];
    let mut after = before.clone();
    after.presentation.states = vec!["focused".into(), "checked".into()];
    assert!(changed_fields(&before, &after).is_empty());
    assert_eq!(RowContent::of(&before), RowContent::of(&after));
}

#[test]
fn content_covers_identity_and_visible_fields() {
    let before = node("button", "OK");
    let mut after = before.clone();
    after.identity.value = Some("1".into());
    assert_ne!(RowContent::of(&before), RowContent::of(&after));
    let mut placeholder = before.clone();
    placeholder.presentation.descriptors.placeholder = Some("hint".into());
    assert_ne!(RowContent::of(&before), RowContent::of(&placeholder));
}
