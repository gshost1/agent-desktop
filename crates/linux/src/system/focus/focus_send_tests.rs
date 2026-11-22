use super::modal_blocks;

const TARGET: u32 = 0x60_0004;
const DIALOG: u32 = 0x60_0010;
const INNER: u32 = 0x60_0020;
const OTHER: u32 = 0x70_0004;
const MODAL: u32 = 401;
const SKIP_TASKBAR: u32 = 402;

#[test]
fn a_modal_dialog_transient_for_the_target_blocks_it() {
    assert!(modal_blocks(
        TARGET,
        DIALOG,
        Some(TARGET),
        &[SKIP_TASKBAR, MODAL],
        MODAL
    ));
}

#[test]
fn a_dialog_over_the_target_that_is_not_modal_does_not_block_it() {
    assert!(!modal_blocks(
        TARGET,
        DIALOG,
        Some(TARGET),
        &[SKIP_TASKBAR],
        MODAL
    ));
    assert!(!modal_blocks(TARGET, DIALOG, Some(TARGET), &[], MODAL));
}

#[test]
fn a_modal_dialog_over_another_window_does_not_block_the_target() {
    assert!(!modal_blocks(TARGET, DIALOG, Some(OTHER), &[MODAL], MODAL));
    assert!(!modal_blocks(TARGET, DIALOG, None, &[MODAL], MODAL));
}

#[test]
fn the_target_itself_active_and_modal_is_not_blocked() {
    assert!(!modal_blocks(TARGET, TARGET, Some(TARGET), &[MODAL], MODAL));
    assert!(!modal_blocks(TARGET, TARGET, Some(OTHER), &[MODAL], MODAL));
}

#[test]
fn a_nested_modal_transient_for_a_dialog_of_the_target_is_left_to_the_request() {
    assert!(!modal_blocks(TARGET, INNER, Some(DIALOG), &[MODAL], MODAL));
}
