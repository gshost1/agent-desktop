use super::*;

#[test]
fn ref_entry_stores_the_window_rectangle_and_its_space() {
    let window_bounds = Rect {
        x: 4.0,
        y: 8.0,
        width: 120.0,
        height: 24.0,
    };
    let screen_bounds = Rect {
        x: 304.0,
        y: 208.0,
        width: 120.0,
        height: 24.0,
    };
    let mut button = node("button", Some("Save"));
    button.presentation.bounds = Some(screen_bounds);
    button.presentation.ref_bounds = Some(window_bounds);

    let entry = ref_entry_from_node(&button, &source(7), None, &[0]);

    assert_eq!(entry.geometry.bounds, Some(window_bounds));
    assert_eq!(entry.geometry.bounds_hash, window_bounds.bounds_hash());
    assert_eq!(
        entry.geometry.bounds_space,
        Some(crate::BoundsSpace::Window)
    );
    assert_eq!(
        entry.geometry.screen_bounds_hash,
        screen_bounds.bounds_hash()
    );
}

#[test]
fn ref_entry_leaves_screen_hash_none_when_only_window_bounds_exist() {
    let window_bounds = Rect {
        x: 4.0,
        y: 8.0,
        width: 120.0,
        height: 24.0,
    };
    let mut button = node("button", Some("Save"));
    button.presentation.bounds = None;
    button.presentation.ref_bounds = Some(window_bounds);

    let entry = ref_entry_from_node(&button, &source(7), None, &[0]);

    assert_eq!(entry.geometry.bounds, Some(window_bounds));
    assert_eq!(entry.geometry.bounds_hash, window_bounds.bounds_hash());
    assert_eq!(
        entry.geometry.bounds_space,
        Some(crate::BoundsSpace::Window)
    );
    assert_eq!(entry.geometry.screen_bounds_hash, None);
}

#[test]
fn ref_entry_stores_the_screen_rectangle_without_a_space_tag() {
    let screen_bounds = Rect {
        x: 304.0,
        y: 208.0,
        width: 120.0,
        height: 24.0,
    };
    let mut button = node("button", Some("Save"));
    button.presentation.bounds = Some(screen_bounds);

    let entry = ref_entry_from_node(&button, &source(7), None, &[0]);

    assert_eq!(entry.geometry.bounds, Some(screen_bounds));
    assert_eq!(entry.geometry.bounds_hash, screen_bounds.bounds_hash());
    assert_eq!(entry.geometry.bounds_space, None);
    assert_eq!(entry.geometry.screen_bounds_hash, None);
}
