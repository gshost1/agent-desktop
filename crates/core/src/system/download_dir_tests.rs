use std::path::Path;

use super::prepare_download_dir;

fn scratch(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "agent-desktop-download-dir-{name}-{}",
        std::process::id()
    ))
}

#[test]
fn a_download_folder_without_cdp_is_refused() {
    let error = prepare_download_dir(Path::new("downloads"), None).unwrap_err();
    assert_eq!(error.code(), "INVALID_ARGS");
}

#[test]
fn a_missing_folder_is_created_and_returned_absolute() {
    let dir = scratch("created").join("nested");
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    let resolved = prepare_download_dir(&dir, Some(0)).unwrap();
    assert!(Path::new(&resolved).is_absolute());
    assert!(dir.is_dir());
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn a_relative_folder_resolves_against_the_working_directory() {
    let resolved = prepare_download_dir(Path::new("target"), Some(0)).unwrap();
    assert_eq!(
        Path::new(&resolved),
        std::env::current_dir().unwrap().join("target")
    );
}

#[test]
fn a_file_in_place_of_the_folder_is_refused() {
    let file = scratch("file");
    std::fs::write(&file, b"x").unwrap();
    let error = prepare_download_dir(&file, Some(0)).unwrap_err();
    assert_eq!(error.code(), "INVALID_ARGS");
    let _ = std::fs::remove_file(&file);
}
