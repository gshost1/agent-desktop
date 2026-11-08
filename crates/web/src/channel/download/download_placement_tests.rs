use super::{DiskState, disk_state, place, safe_file_name};
use crate::channel::test_dir::TestDir;

const GUID: &str = "0bbf3529-01aa-49fa-8191-e85040f21a22";

#[test]
fn the_folder_tells_a_partial_a_finished_and_a_missing_download_apart() {
    let dir = TestDir::new();
    assert_eq!(disk_state(dir.path(), GUID), DiskState::Missing);
    std::fs::write(dir.path().join(format!("{GUID}.crdownload")), b"a,").unwrap();
    assert_eq!(disk_state(dir.path(), GUID), DiskState::Partial);
    std::fs::write(dir.path().join(GUID), b"a,b\n").unwrap();
    assert_eq!(disk_state(dir.path(), GUID), DiskState::Finished(4));
}

#[test]
fn a_guid_that_is_not_a_plain_name_never_reads_outside_the_folder() {
    let dir = TestDir::new();
    std::fs::write(dir.path().join("secret"), b"x").unwrap();
    let inner = dir.path().join("inner");
    std::fs::create_dir(&inner).unwrap();
    assert_eq!(disk_state(&inner, "../secret"), DiskState::Missing);
    assert!(place(&inner, "../secret", "a.csv").is_err());
    assert!(dir.path().join("secret").exists());
}

#[test]
fn a_finished_download_takes_its_suggested_name_without_replacing_a_file() {
    let dir = TestDir::new();
    std::fs::write(dir.path().join("report.csv"), b"older").unwrap();
    std::fs::write(dir.path().join(GUID), b"newer").unwrap();
    let placed = place(dir.path(), GUID, "report.csv").unwrap();
    assert_eq!(placed, dir.path().join("report (1).csv"));
    assert_eq!(std::fs::read(&placed).unwrap(), b"newer");
    assert_eq!(
        std::fs::read(dir.path().join("report.csv")).unwrap(),
        b"older"
    );
    assert!(!dir.path().join(GUID).exists());
}

#[test]
fn suggested_names_cannot_leave_the_folder_or_break_windows_paths() {
    assert_eq!(safe_file_name("../../etc/passwd"), "passwd");
    assert_eq!(safe_file_name("..\\..\\boot.ini"), "boot.ini");
    assert_eq!(safe_file_name("C:report?.csv"), "C_report_.csv");
    assert_eq!(safe_file_name("a\u{0}b.csv"), "a_b.csv");
    assert_eq!(safe_file_name(".."), "download");
    assert_eq!(safe_file_name(""), "download");
    assert_eq!(safe_file_name("trailing. "), "trailing");
    assert_eq!(safe_file_name("NUL.txt"), "_NUL.txt");
}
