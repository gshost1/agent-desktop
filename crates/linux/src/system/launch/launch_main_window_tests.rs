use super::*;

fn candidate(normal: Option<bool>, area: u32, order: usize, title: Option<&str>) -> Candidate {
    Candidate {
        normal,
        transient: false,
        area,
        order,
        title: title.map(str::to_owned),
    }
}

#[test]
fn a_splash_before_a_normal_window_does_not_supply_the_title() {
    assert_eq!(
        select_title(&[
            candidate(Some(false), 1000, 0, Some("Splash")),
            candidate(Some(true), 500, 1, Some("Editor")),
        ]),
        Some("Editor".into())
    );
}

#[test]
fn the_largest_normal_window_supplies_the_title() {
    assert_eq!(
        select_title(&[
            candidate(Some(true), 100, 0, Some("Small")),
            candidate(Some(true), 500, 1, Some("Main")),
            candidate(Some(true), 200, 2, Some("Recent")),
        ]),
        Some("Main".into())
    );
}

#[test]
fn equal_area_windows_use_the_newest_when_their_titles_agree() {
    let candidates = [
        candidate(Some(true), 500, 2, Some("Editor")),
        candidate(Some(true), 500, 1, Some("Editor")),
    ];
    assert_eq!(select_largest(&candidates).unwrap().order, 2);
    assert_eq!(select_title(&candidates), Some("Editor".into()));
}

#[test]
fn legacy_only_and_empty_titles_are_untrusted() {
    for title in [None, Some(""), Some(" ")] {
        assert_eq!(select_title(&[candidate(Some(true), 500, 0, title)]), None);
    }
}

#[test]
fn equal_area_windows_with_different_or_missing_titles_are_untrusted() {
    for title in [Some("Other"), None] {
        assert_eq!(
            select_title(&[
                candidate(Some(true), 500, 0, Some("Editor")),
                candidate(Some(true), 500, 1, title),
            ]),
            None
        );
    }
}

#[test]
fn no_normal_window_has_no_title() {
    assert_eq!(select_title(&[]), None);
    assert_eq!(
        select_title(&[candidate(Some(false), 500, 0, Some("Splash"))]),
        None
    );
}

#[test]
fn an_absent_type_defaults_to_normal_only_without_a_transient_parent() {
    let mut window = candidate(None, 500, 0, Some("Editor"));
    assert_eq!(
        select_title(std::slice::from_ref(&window)),
        Some("Editor".into())
    );
    window.transient = true;
    assert_eq!(select_title(&[window]), None);
}
