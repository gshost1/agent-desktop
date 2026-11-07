use super::title_changed;

#[test]
fn only_a_new_readable_title_counts_as_a_change() {
    assert!(title_changed("Downloads", Some("folder-7")));
    assert!(!title_changed("Downloads", Some("Downloads")));
    assert!(!title_changed("Downloads", None));
    assert!(!title_changed("Downloads", Some("  ")));
    assert!(!title_changed("", Some("folder-7")));
}
