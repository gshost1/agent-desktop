use super::retitled_after_target;

fn labels(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| name.to_string()).collect()
}

#[test]
fn a_new_title_naming_the_target_counts_as_the_open() {
    assert!(retitled_after_target(
        "Downloads",
        Some("folder-7"),
        &labels(&["folder-7"])
    ));
    assert!(retitled_after_target(
        "Downloads",
        Some(" folder-7 "),
        &labels(&["", "folder-7"])
    ));
}

#[test]
fn an_unchanged_title_is_no_change() {
    assert!(!retitled_after_target(
        "folder-7",
        Some("folder-7"),
        &labels(&["folder-7"])
    ));
}

#[test]
fn a_retitle_that_does_not_name_the_target_proves_nothing() {
    assert!(!retitled_after_target(
        "Downloads",
        Some("Downloads \u{2014} Edited"),
        &labels(&["folder-7"])
    ));
    assert!(!retitled_after_target(
        "Downloads",
        Some("folder-8"),
        &labels(&["folder-7"])
    ));
}

#[test]
fn an_unreadable_or_blank_title_proves_nothing() {
    assert!(!retitled_after_target(
        "Downloads",
        None,
        &labels(&["folder-7"])
    ));
    assert!(!retitled_after_target(
        "Downloads",
        Some("  "),
        &labels(&["folder-7"])
    ));
}
