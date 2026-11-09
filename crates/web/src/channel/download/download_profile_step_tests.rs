use std::collections::HashSet;

use agent_desktop_core::ErrorCode;

use super::next_profile_step;
use crate::channel::download::download_record::DownloadRecord;
use crate::channel::download::download_state::DownloadState;
use crate::channel::download::download_step::DownloadStep;
use crate::channel::download::process_downloads::ProcessDownloads;
use crate::channel::test_dir::TestDir;

const GUID: &str = "0bbf3529-01aa-49fa-8191-e85040f21a22";

fn entry(records: Vec<DownloadRecord>) -> ProcessDownloads {
    ProcessDownloads {
        pid: 1,
        process_instance: "one".into(),
        records,
        claimed: Vec::new(),
    }
}

fn record(state: DownloadState, path: Option<String>) -> DownloadRecord {
    DownloadRecord {
        guid: GUID.into(),
        url: "http://127.0.0.1/report.csv".into(),
        suggested_filename: "report.csv".into(),
        state,
        bytes: 0,
        path,
        delivered: false,
    }
}

fn step(entry: &mut ProcessDownloads, dir: &TestDir, grace_over: bool) -> DownloadStep {
    next_profile_step(entry, dir.path(), &HashSet::new(), grace_over)
}

fn ready(step: DownloadStep) -> agent_desktop_core::DownloadInfo {
    match step {
        DownloadStep::Ready(info) => info,
        _ => panic!("expected a saved download"),
    }
}

fn failure_kind(step: DownloadStep) -> String {
    match step {
        DownloadStep::Failed(error) => {
            assert_eq!(error.code, ErrorCode::ActionFailed);
            error.details.unwrap()["kind"].as_str().unwrap().to_owned()
        }
        _ => panic!("expected a failed download"),
    }
}

#[test]
fn a_file_no_connection_saw_begin_is_returned_once_under_its_own_name() {
    let dir = TestDir::new();
    std::fs::write(dir.path().join("report.csv"), b"a,b\n").unwrap();
    let mut downloads = entry(Vec::new());

    let info = ready(step(&mut downloads, &dir, false));

    assert_eq!(info.path, dir.path().join("report.csv").to_string_lossy());
    assert_eq!(info.suggested_filename, "report.csv");
    assert_eq!(info.url, None);
    assert_eq!(info.bytes, 4);
    assert!(matches!(
        step(&mut downloads, &dir, false),
        DownloadStep::Idle
    ));
}

#[test]
fn a_recorded_download_is_matched_to_the_numbered_copy_the_browser_saved() {
    let dir = TestDir::new();
    std::fs::write(dir.path().join("report.csv"), b"old").unwrap();
    std::fs::write(dir.path().join("report (1).csv"), b"a,b\n").unwrap();
    let mut downloads = entry(vec![record(DownloadState::InProgress, None)]);
    downloads.claim("report.csv");

    let info = ready(step(&mut downloads, &dir, false));

    assert_eq!(
        info.path,
        dir.path().join("report (1).csv").to_string_lossy()
    );
    assert_eq!(info.url.as_deref(), Some("http://127.0.0.1/report.csv"));
    assert_eq!(info.suggested_filename, "report.csv");
    assert!(downloads.records[0].delivered);
    assert!(downloads.is_claimed("report (1).csv"));
}

#[test]
fn a_file_that_was_there_at_launch_is_never_a_download() {
    let dir = TestDir::new();
    std::fs::write(dir.path().join("notes.txt"), b"mine").unwrap();
    let mut downloads = entry(Vec::new());
    downloads.claim("notes.txt");

    assert!(matches!(
        step(&mut downloads, &dir, true),
        DownloadStep::Idle
    ));
}

#[test]
fn a_partial_file_is_a_download_still_receiving() {
    let dir = TestDir::new();
    std::fs::write(dir.path().join("slow.csv.crdownload"), b"a").unwrap();

    match step(&mut entry(Vec::new()), &dir, true) {
        DownloadStep::Pending(record) => {
            assert_eq!(record.suggested_filename, "slow.csv");
            assert!(record.url.is_empty());
        }
        _ => panic!("expected a pending download"),
    }
    let mut recorded = entry(vec![record(DownloadState::InProgress, None)]);
    assert!(matches!(
        step(&mut recorded, &dir, true),
        DownloadStep::Pending(_)
    ));
}

#[test]
fn a_recorded_download_with_no_file_is_interrupted_after_the_grace() {
    let dir = TestDir::new();
    let mut downloads = entry(vec![record(DownloadState::InProgress, None)]);

    assert!(matches!(
        step(&mut downloads, &dir, false),
        DownloadStep::Missing
    ));
    assert_eq!(
        failure_kind(step(&mut downloads, &dir, true)),
        "cdp_download_interrupted"
    );
    assert!(downloads.records[0].delivered);
}

#[test]
fn a_download_this_connection_follows_waits_for_its_events() {
    let dir = TestDir::new();
    let mut downloads = entry(vec![record(DownloadState::InProgress, None)]);
    let seen = HashSet::from([GUID.to_owned()]);

    let step = next_profile_step(&mut downloads, dir.path(), &seen, true);

    assert!(matches!(step, DownloadStep::Pending(_)));
}

#[test]
fn a_canceled_download_is_an_error_not_a_path() {
    let dir = TestDir::new();
    let mut downloads = entry(vec![record(DownloadState::Canceled, None)]);

    assert_eq!(
        failure_kind(step(&mut downloads, &dir, false)),
        "cdp_download_canceled"
    );
}

#[test]
fn a_download_whose_events_named_its_file_claims_that_file() {
    let dir = TestDir::new();
    let saved = dir.path().join("report.csv");
    std::fs::write(&saved, b"a,b\n").unwrap();
    let path = saved.to_string_lossy().into_owned();
    let mut downloads = entry(vec![record(DownloadState::Completed, Some(path.clone()))]);

    assert_eq!(ready(step(&mut downloads, &dir, false)).path, path);
    assert!(matches!(
        step(&mut downloads, &dir, false),
        DownloadStep::Idle
    ));
}
