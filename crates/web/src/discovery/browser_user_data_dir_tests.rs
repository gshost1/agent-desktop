use std::ffi::OsString;
use std::path::PathBuf;

use super::user_data_dirs_with;

fn fake_env(name: &str) -> Option<OsString> {
    match name {
        "HOME" => Some(OsString::from("/home/tester")),
        "LOCALAPPDATA" => Some(OsString::from("C:/Users/tester/AppData/Local")),
        _ => None,
    }
}

#[test]
fn every_browser_has_one_location_under_the_os_base() {
    let dirs = user_data_dirs_with(fake_env);
    let names: Vec<&str> = dirs.iter().map(|entry| entry.browser).collect();
    assert_eq!(
        names,
        ["chrome", "chrome-canary", "chromium", "edge", "brave"]
    );
    for entry in &dirs {
        assert!(entry.path.starts_with(expected_base()), "{entry:?}");
    }
}

#[cfg(target_os = "macos")]
fn expected_base() -> PathBuf {
    PathBuf::from("/home/tester/Library/Application Support")
}

#[cfg(target_os = "linux")]
fn expected_base() -> PathBuf {
    PathBuf::from("/home/tester/.config")
}

#[cfg(target_os = "windows")]
fn expected_base() -> PathBuf {
    PathBuf::from("C:/Users/tester/AppData/Local")
}

#[cfg(target_os = "macos")]
#[test]
fn chrome_resolves_to_the_application_support_profile() {
    let chrome = user_data_dirs_with(fake_env)
        .into_iter()
        .find(|entry| entry.browser == "chrome")
        .unwrap();
    assert_eq!(
        chrome.path,
        PathBuf::from("/home/tester/Library/Application Support/Google/Chrome")
    );
}

#[test]
fn no_base_directory_yields_no_defaults() {
    assert!(user_data_dirs_with(|_| None).is_empty());
}
