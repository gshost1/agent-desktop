use super::{FolderListing, saved_as};
use crate::channel::test_dir::TestDir;

#[test]
fn a_saved_name_is_the_suggested_one_or_the_browser_numbered_copy() {
    assert!(saved_as("report.csv", "report.csv"));
    assert!(saved_as("report.csv", "report (1).csv"));
    assert!(saved_as("report.csv", "report (12).csv"));
    assert!(saved_as("README", "README (2)"));
    assert!(saved_as("archive.tar.gz", "archive.tar (1).gz"));
    assert!(!saved_as("report.csv", "report (x).csv"));
    assert!(!saved_as("report.csv", "report ().csv"));
    assert!(!saved_as("report.csv", "other.csv"));
    assert!(!saved_as("report.csv", "report.csv.crdownload"));
}

#[test]
fn a_listing_splits_finished_files_from_partial_ones_and_skips_hidden_files() {
    let dir = TestDir::new();
    std::fs::write(dir.path().join("report.csv"), b"a,b\n").unwrap();
    std::fs::write(dir.path().join("slow.csv.crdownload"), b"a").unwrap();
    std::fs::write(dir.path().join(".DS_Store"), b"").unwrap();
    std::fs::create_dir(dir.path().join("nested")).unwrap();

    let listing = FolderListing::read(dir.path());

    let names: Vec<&str> = listing
        .finished
        .iter()
        .map(|file| file.name.as_str())
        .collect();
    assert_eq!(names, vec!["report.csv"]);
    assert_eq!(listing.finished[0].bytes, 4);
    assert_eq!(listing.partial, vec!["slow.csv".to_owned()]);
}

#[test]
fn a_missing_folder_lists_nothing() {
    let dir = TestDir::new();
    assert_eq!(
        FolderListing::read(&dir.path().join("absent")),
        FolderListing::default()
    );
}
