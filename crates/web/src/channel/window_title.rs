const SEPARATORS: [char; 5] = ['-', '\u{2013}', '\u{2014}', '|', '\u{00b7}'];

/// Whether a platform window titled `window_title` shows a tab titled
/// `tab_title`. A tabbed browser titles each window after its active tab,
/// either alone or followed by a spaced separator and more text such as the
/// browser or profile name. Page visibility cannot stand in for this: a
/// browser reports every tab of a covered or minimized window as hidden,
/// while the window keeps its active tab's title.
pub(crate) fn shows_tab_title(window_title: &str, tab_title: &str) -> bool {
    let window_title = window_title.trim();
    let tab_title = tab_title.trim();
    if tab_title.is_empty() {
        return false;
    }
    let Some(rest) = window_title.strip_prefix(tab_title) else {
        return false;
    };
    if rest.is_empty() {
        return true;
    }
    let after_space = rest.trim_start();
    after_space.len() < rest.len() && after_space.starts_with(SEPARATORS)
}

#[cfg(test)]
#[path = "window_title_tests.rs"]
mod tests;
