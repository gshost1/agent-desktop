use std::path::PathBuf;

use agent_desktop_core::DownloadScope;
use serde_json::{Value, json};

use super::{point_downloads_at, preferences_path, save_in_profile};
use crate::channel::test_dir::TestDir;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|arg| (*arg).to_owned()).collect()
}

fn read(path: &std::path::Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn a_fresh_profile_gets_the_folder_as_its_download_preference() {
    let root = TestDir::new();
    let profile = root.path().join("chrome-profile");
    let downloads = root.path().join("downloads");
    let launch = args(&[
        &format!("--user-data-dir={}", profile.display()),
        "--no-first-run",
    ]);

    let scope = save_in_profile(&launch, &downloads).unwrap();

    assert_eq!(scope, DownloadScope::Profile);
    let prefs = read(&profile.join("Default").join("Preferences"));
    assert_eq!(prefs["download"]["default_directory"], json!(downloads));
    assert_eq!(prefs["download"]["prompt_for_download"], false);
    assert_eq!(prefs["savefile"]["default_directory"], json!(downloads));
}

#[test]
fn an_existing_profile_keeps_every_other_preference() {
    let root = TestDir::new();
    let profile = root.path().join("chrome-profile");
    let preferences = profile.join("Work").join("Preferences");
    std::fs::create_dir_all(preferences.parent().unwrap()).unwrap();
    std::fs::write(
        &preferences,
        br#"{"homepage":"https://example.test","download":{"extensions_to_open":"pdf"}}"#,
    )
    .unwrap();
    let downloads = root.path().join("downloads");
    let launch = args(&[
        &format!("--user-data-dir={}", profile.display()),
        "--profile-directory=Work",
    ]);

    assert_eq!(
        save_in_profile(&launch, &downloads).unwrap(),
        DownloadScope::Profile
    );

    let prefs = read(&preferences);
    assert_eq!(prefs["homepage"], "https://example.test");
    assert_eq!(prefs["download"]["extensions_to_open"], "pdf");
    assert_eq!(prefs["download"]["default_directory"], json!(downloads));
}

#[test]
fn a_preferences_file_that_is_not_json_is_left_alone() {
    let root = TestDir::new();
    let profile = root.path().join("chrome-profile");
    let preferences = profile.join("Default").join("Preferences");
    std::fs::create_dir_all(preferences.parent().unwrap()).unwrap();
    std::fs::write(&preferences, b"{ not json").unwrap();
    let launch = args(&[&format!("--user-data-dir={}", profile.display())]);

    let scope = save_in_profile(&launch, &root.path().join("downloads")).unwrap();

    assert_eq!(scope, DownloadScope::Connection);
    assert_eq!(std::fs::read(&preferences).unwrap(), b"{ not json");
}

#[test]
fn only_an_absolute_user_data_dir_that_is_no_browser_default_is_written() {
    let defaults = vec![PathBuf::from(
        "/Users/me/Library/Application Support/Google/Chrome",
    )];

    assert_eq!(
        preferences_path(&args(&["--no-first-run"]), &defaults),
        None
    );
    assert_eq!(
        preferences_path(&args(&["--user-data-dir=chrome-profile"]), &defaults),
        None
    );
    assert_eq!(
        preferences_path(
            &args(&["--user-data-dir=/Users/me/Library/Application Support/Google/Chrome"]),
            &defaults
        ),
        None
    );
    assert_eq!(
        preferences_path(
            &args(&["--user-data-dir=/tmp/p", "--profile-directory=../escape"]),
            &defaults
        ),
        None
    );
    assert_eq!(
        preferences_path(
            &args(&["--user-data-dir=/tmp/old", "--user-data-dir=/tmp/p"]),
            &defaults
        ),
        Some(PathBuf::from("/tmp/p/Default/Preferences"))
    );
}

#[test]
fn a_section_of_the_wrong_type_is_replaced() {
    let mut document = json!({ "download": "broken", "savefile": 3 });

    point_downloads_at(&mut document, std::path::Path::new("/tmp/d"));

    assert_eq!(document["download"]["default_directory"], "/tmp/d");
    assert_eq!(document["savefile"]["default_directory"], "/tmp/d");
}
