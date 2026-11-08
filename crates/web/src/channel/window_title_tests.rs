use super::shows_tab_title;

#[test]
fn a_window_titled_exactly_like_the_tab_shows_it() {
    assert!(shows_tab_title("Docs", "Docs"));
}

#[test]
fn a_window_title_with_a_spaced_suffix_shows_the_tab() {
    assert!(shows_tab_title("Docs - Browser", "Docs"));
    assert!(shows_tab_title("Docs \u{2014} Profile 1", "Docs"));
}

#[test]
fn a_longer_word_or_an_unspaced_suffix_does_not_show_the_tab() {
    assert!(!shows_tab_title("Docstring", "Docs"));
    assert!(!shows_tab_title("Docs-Guide", "Docs"));
    assert!(!shows_tab_title("Docs Guide", "Docs"));
}

#[test]
fn an_empty_tab_title_shows_in_no_window() {
    assert!(!shows_tab_title("Docs", ""));
    assert!(!shows_tab_title("", ""));
}

#[test]
fn a_tab_whose_title_is_not_the_window_prefix_is_not_shown() {
    assert!(!shows_tab_title("Mail - Browser", "Docs"));
}
