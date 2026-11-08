use super::first_focused_root;
use agent_desktop_core::{AdapterError, ErrorCode};
use std::cell::Cell;

#[test]
fn a_focused_editor_is_the_focused_surface_while_no_window_reports_focus() {
    let root = first_focused_root(|| Ok(None), || Ok(Some("editor"))).unwrap();

    assert_eq!(root, Some("editor"));
}

#[test]
fn a_focused_window_wins_and_the_editor_is_not_read() {
    let editor_read = Cell::new(false);
    let root = first_focused_root(
        || Ok(Some("window")),
        || {
            editor_read.set(true);
            Ok(Some("editor"))
        },
    )
    .unwrap();

    assert_eq!(root, Some("window"));
    assert!(!editor_read.get());
}

#[test]
fn a_failed_window_read_is_not_masked_by_the_editor() {
    let error = first_focused_root(
        || Err(AdapterError::new(ErrorCode::PermDenied, "denied")),
        || Ok(Some("editor")),
    )
    .unwrap_err();

    assert_eq!(error.code, ErrorCode::PermDenied);
    assert_eq!(
        first_focused_root::<&str>(|| Ok(None), || Ok(None)).unwrap(),
        None
    );
}
