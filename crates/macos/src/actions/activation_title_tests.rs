use super::title_changed;

#[test]
fn a_new_readable_title_counts_as_a_change() {
    assert!(title_changed("Downloads", Some("folder-7")));
}

#[test]
fn an_unchanged_title_is_no_change() {
    assert!(!title_changed("Downloads", Some("Downloads")));
}

#[test]
fn an_unreadable_or_blank_title_proves_nothing() {
    assert!(!title_changed("Downloads", None));
    assert!(!title_changed("Downloads", Some("  ")));
    assert!(!title_changed("", Some("folder-7")));
    assert!(!title_changed("   ", Some("folder-7")));
}
