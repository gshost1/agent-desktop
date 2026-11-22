use super::*;

#[test]
fn space_functions_resolve_on_this_macos() {
    assert!(space_functions().is_some());
}

#[test]
fn a_window_number_with_no_window_is_on_no_space() {
    assert_eq!(is_on_any_space(i64::from(u32::MAX)), Some(false));
}
