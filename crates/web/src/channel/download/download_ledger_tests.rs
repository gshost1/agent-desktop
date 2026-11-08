use agent_desktop_core::{ProcessIdentity, StateFile};

use super::update_records;
use crate::channel::download::download_record::DownloadRecord;
use crate::channel::download::download_state::DownloadState;
use crate::channel::test_dir::TestDir;
use crate::channel::test_support::deadline;

fn record(guid: &str, delivered: bool) -> DownloadRecord {
    DownloadRecord {
        guid: guid.into(),
        url: format!("http://127.0.0.1/{guid}"),
        suggested_filename: format!("{guid}.csv"),
        state: DownloadState::Completed,
        bytes: 1,
        path: None,
        delivered,
    }
}

fn guids(file: &StateFile, process: &ProcessIdentity) -> Vec<String> {
    update_records(file, process, deadline(), |records| {
        records.iter().map(|record| record.guid.clone()).collect()
    })
    .unwrap()
}

#[test]
fn records_persist_per_browser_process() {
    let dir = TestDir::new();
    let file = StateFile::at(dir.path().join("downloads.json"));
    let first = ProcessIdentity::new(1u32, "one");
    let second = ProcessIdentity::new(2u32, "two");
    update_records(&file, &first, deadline(), |records| {
        records.push(record("a", false))
    })
    .unwrap();
    update_records(&file, &second, deadline(), |records| {
        records.push(record("b", false))
    })
    .unwrap();
    assert_eq!(guids(&file, &first), vec!["a"]);
    assert_eq!(guids(&file, &second), vec!["b"]);
    let restarted = ProcessIdentity::new(1u32, "one-again");
    assert!(guids(&file, &restarted).is_empty());
}

#[test]
fn a_full_ledger_drops_delivered_downloads_before_waiting_ones() {
    let dir = TestDir::new();
    let file = StateFile::at(dir.path().join("downloads.json"));
    let process = ProcessIdentity::new(1u32, "one");
    update_records(&file, &process, deadline(), |records| {
        records.push(record("waiting", false));
        for index in 0..32 {
            records.push(record(&format!("done{index}"), true));
        }
    })
    .unwrap();
    let kept = guids(&file, &process);
    assert_eq!(kept.len(), 32);
    assert_eq!(kept[0], "waiting");
    assert!(!kept.contains(&"done0".to_owned()));
}

#[test]
fn a_malformed_ledger_reads_as_empty_and_is_rewritten() {
    let dir = TestDir::new();
    let file = StateFile::at(dir.path().join("downloads.json"));
    file.write(b"{not json").unwrap();
    let process = ProcessIdentity::new(1u32, "one");
    assert!(guids(&file, &process).is_empty());
    update_records(&file, &process, deadline(), |records| {
        records.push(record("a", false))
    })
    .unwrap();
    assert_eq!(guids(&file, &process), vec!["a"]);
}
