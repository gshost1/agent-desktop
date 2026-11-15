//! Drives real Win32 edits and combo boxes through `set_value_steps` and
//! checks that their owner hears the change.

use super::test_support::{
    CBN_EDITCHANGE, COMBO_ID, DISABLED_ID, DROP_LIST_ID, EN_CHANGE, EditOwner, FIXED_WIDTH_ID,
    LIMITED_ID, LIMITED_LENGTH, MULTILINE_ID, READ_ONLY_ID, SINGLE_LINE_ID, deadline, heard,
    text_of, write,
};
use super::{EDIT_LABEL, EditTarget, edit_target};
use crate::actions::value_write::clear_steps;
use crate::tree::automation::root_from_hwnd;
use crate::tree::fixture::ensure_test_apartment;
use agent_desktop_core::InteractionPolicy;

#[test]
fn set_value_on_a_combo_box_edit_tells_the_combo_box_owner() {
    ensure_test_apartment();
    let owner = EditOwner::create();
    let combo = owner.control(COMBO_ID);
    let steps = write(owner.combo_edit(), r"C:\out\written-name.txt");
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].label(), EDIT_LABEL);
    assert_eq!(steps[0].verified(), Some(true));
    assert_eq!(text_of(owner.combo_edit()), r"C:\out\written-name.txt");
    assert!(
        heard(combo, CBN_EDITCHANGE),
        "the owner heard CBN_EDITCHANGE"
    );
}

#[test]
fn set_value_on_the_combo_box_itself_writes_its_edit() {
    ensure_test_apartment();
    let owner = EditOwner::create();
    let combo = owner.control(COMBO_ID);
    let steps = write(combo, "through the combo box");
    assert_eq!(steps[0].label(), EDIT_LABEL);
    assert_eq!(text_of(owner.combo_edit()), "through the combo box");
    assert!(heard(combo, CBN_EDITCHANGE));
}

#[test]
fn set_value_on_a_multiline_edit_sends_en_change() {
    ensure_test_apartment();
    let owner = EditOwner::create();
    let edit = owner.control(MULTILINE_ID);
    let steps = write(edit, "first line");
    assert_eq!(steps[0].label(), EDIT_LABEL);
    assert_eq!(text_of(edit), "first line");
    assert!(heard(edit, EN_CHANGE), "the owner heard EN_CHANGE");
}

#[test]
fn set_value_on_a_single_line_edit_sends_en_change() {
    ensure_test_apartment();
    let owner = EditOwner::create();
    let edit = owner.control(SINGLE_LINE_ID);
    let steps = write(edit, "single line");
    assert_eq!(steps[0].label(), EDIT_LABEL);
    assert_eq!(steps[0].verified(), Some(true));
    assert!(heard(edit, EN_CHANGE));
}

#[test]
fn a_value_pattern_write_to_a_combo_box_edit_tells_the_owner_nothing() {
    use uiautomation::patterns::UIValuePattern;
    ensure_test_apartment();
    let owner = EditOwner::create();
    let combo = owner.control(COMBO_ID);
    let element = root_from_hwnd(owner.combo_edit(), deadline()).expect("the edit resolves");
    element
        .0
        .get_pattern::<UIValuePattern>()
        .expect("the edit advertises ValuePattern")
        .set_value("silent write")
        .expect("ValuePattern.SetValue succeeds");
    assert_eq!(text_of(owner.combo_edit()), "silent write");
    assert!(
        !heard(combo, CBN_EDITCHANGE),
        "WM_SETTEXT on a combo box edit sends no change notification"
    );
}

#[test]
fn a_value_pattern_write_to_a_multiline_edit_tells_the_owner_nothing() {
    use uiautomation::patterns::UIValuePattern;
    ensure_test_apartment();
    let owner = EditOwner::create();
    let edit = owner.control(MULTILINE_ID);
    let element = root_from_hwnd(edit, deadline()).expect("the edit resolves");
    element
        .0
        .get_pattern::<UIValuePattern>()
        .expect("the edit advertises ValuePattern")
        .set_value("silent write")
        .expect("ValuePattern.SetValue succeeds");
    assert_eq!(text_of(edit), "silent write");
    assert!(
        !heard(edit, EN_CHANGE),
        "WM_SETTEXT on a multiline edit sends no change notification"
    );
}

#[test]
fn clear_on_a_combo_box_edit_tells_the_combo_box_owner() {
    ensure_test_apartment();
    let owner = EditOwner::create();
    let combo = owner.control(COMBO_ID);
    let element = root_from_hwnd(owner.combo_edit(), deadline()).expect("the edit resolves");
    let steps =
        clear_steps(&element, InteractionPolicy::headless(), deadline()).expect("clear delivers");
    assert_eq!(steps[0].label(), EDIT_LABEL);
    assert_eq!(text_of(owner.combo_edit()), "");
    assert!(heard(combo, CBN_EDITCHANGE));
}

#[test]
fn read_only_edits_and_drop_down_lists_keep_their_value_pattern() {
    ensure_test_apartment();
    let owner = EditOwner::create();
    for id in [READ_ONLY_ID, DROP_LIST_ID] {
        let element = root_from_hwnd(owner.control(id), deadline()).expect("resolves");
        assert_eq!(
            edit_target(&element, "value", deadline()),
            EditTarget::NotEdit,
            "control {id}"
        );
    }
    let element = root_from_hwnd(owner.control(MULTILINE_ID), deadline()).expect("resolves");
    assert_eq!(
        edit_target(&element, "with\0nul", deadline()),
        EditTarget::NotEdit,
        "a value holding U+0000 keeps ValuePattern"
    );
    let element = root_from_hwnd(owner.control(SINGLE_LINE_ID), deadline()).expect("resolves");
    assert_eq!(
        edit_target(&element, "value", deadline()),
        EditTarget::Edit(owner.control(SINGLE_LINE_ID))
    );
}

#[test]
fn an_edit_that_cannot_scroll_keeps_the_value_pattern_and_its_whole_value() {
    ensure_test_apartment();
    let owner = EditOwner::create();
    let edit = owner.control(FIXED_WIDTH_ID);
    let wide_value = "a value much wider than the three hundred pixel box it is written to, so typing would stop at its edge";
    let element = root_from_hwnd(edit, deadline()).expect("resolves");
    assert_eq!(
        edit_target(&element, wide_value, deadline()),
        EditTarget::NotEdit
    );
    let steps = write(edit, wide_value);
    assert_ne!(steps[0].label(), EDIT_LABEL);
    assert_eq!(text_of(edit), wide_value);
}

#[test]
fn a_disabled_edit_keeps_its_value_pattern() {
    ensure_test_apartment();
    let owner = EditOwner::create();
    let element = root_from_hwnd(owner.control(DISABLED_ID), deadline()).expect("resolves");
    assert_eq!(
        edit_target(&element, "value", deadline()),
        EditTarget::NotEdit,
        "a disabled edit refuses typing, so EM_REPLACESEL would not land"
    );
}

#[test]
fn a_value_over_the_text_limit_keeps_the_value_pattern_and_one_at_the_limit_does_not() {
    ensure_test_apartment();
    let owner = EditOwner::create();
    let edit = owner.control(LIMITED_ID);
    let element = root_from_hwnd(edit, deadline()).expect("resolves");
    let at_limit = "x".repeat(LIMITED_LENGTH);
    let over_limit = "y".repeat(LIMITED_LENGTH + 1);
    assert_eq!(
        edit_target(&element, &at_limit, deadline()),
        EditTarget::Edit(edit)
    );
    assert_eq!(
        edit_target(&element, &over_limit, deadline()),
        EditTarget::NotEdit,
        "EM_REPLACESEL would cut a value over EM_GETLIMITTEXT"
    );
    let steps = write(edit, &over_limit);
    assert_ne!(steps[0].label(), EDIT_LABEL);
    assert_eq!(
        text_of(edit),
        over_limit,
        "ValuePattern writes the whole value"
    );
}
