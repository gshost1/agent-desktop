use super::asked;
use std::ffi::OsStr;

fn args(list: &'static [&'static str]) -> impl Iterator<Item = &'static OsStr> {
    list.iter().map(OsStr::new)
}

#[test]
fn the_variable_set_to_one_opts_in() {
    assert!(asked(Some(OsStr::new("1")), args(&["version"])));
}

#[test]
fn any_other_value_or_none_stays_in_process() {
    assert!(!asked(Some(OsStr::new("0")), args(&["version"])));
    assert!(!asked(Some(OsStr::new("true")), args(&["version"])));
    assert!(!asked(None, args(&["version"])));
}

#[test]
fn the_flag_opts_in_anywhere_before_the_separator() {
    assert!(asked(None, args(&["--daemon", "version"])));
    assert!(asked(
        None,
        args(&["snapshot", "--app", "Files", "--daemon"])
    ));
}

#[test]
fn a_flag_after_the_separator_is_an_argument_not_an_opt_in() {
    assert!(!asked(None, args(&["type", "@s1:e1", "--", "--daemon"])));
}
