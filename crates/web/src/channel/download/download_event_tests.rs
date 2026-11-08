use serde_json::{Value, json};

use super::{apply_event, is_download_event, is_download_start};
use crate::CdpEvent;
use crate::channel::download::download_record::DownloadRecord;
use crate::channel::download::download_state::DownloadState;

pub(crate) fn begin(guid: &str, name: &str) -> CdpEvent {
    browser_event(
        "Browser.downloadWillBegin",
        json!({ "guid": guid, "url": format!("http://127.0.0.1/{name}"), "suggestedFilename": name, "frameId": "F" }),
    )
}

pub(crate) fn progress(guid: &str, state: &str, bytes: u64, path: Option<&str>) -> CdpEvent {
    let mut params =
        json!({ "guid": guid, "state": state, "receivedBytes": bytes, "totalBytes": bytes });
    if let Some(path) = path {
        params["filePath"] = json!(path);
    }
    browser_event("Browser.downloadProgress", params)
}

fn browser_event(method: &str, params: Value) -> CdpEvent {
    CdpEvent {
        method: method.into(),
        params,
        session_id: None,
    }
}

fn fold(events: &[CdpEvent]) -> Vec<DownloadRecord> {
    let mut records = Vec::new();
    for event in events {
        apply_event(&mut records, event);
    }
    records
}

#[test]
fn a_start_then_completion_records_the_saved_file() {
    let records = fold(&[
        begin("g1", "report.csv"),
        progress("g1", "inProgress", 4, None),
        progress("g1", "completed", 8, Some("/dl/g1")),
    ]);
    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.suggested_filename, "report.csv");
    assert_eq!(record.url, "http://127.0.0.1/report.csv");
    assert_eq!(record.state, DownloadState::Completed);
    assert_eq!(record.bytes, 8);
    assert_eq!(record.path.as_deref(), Some("/dl/g1"));
    assert!(!record.delivered);
}

#[test]
fn a_canceled_download_stays_canceled_when_late_progress_arrives() {
    let records = fold(&[
        begin("g1", "a.csv"),
        progress("g1", "canceled", 0, None),
        progress("g1", "inProgress", 4, None),
    ]);
    assert_eq!(records[0].state, DownloadState::Canceled);
    assert_eq!(records[0].bytes, 0);
}

#[test]
fn a_repeated_start_does_not_duplicate_and_unknown_progress_is_ignored() {
    let records = fold(&[
        begin("g1", "a.csv"),
        begin("g1", "a.csv"),
        progress("other", "completed", 3, Some("/dl/other")),
    ]);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].state, DownloadState::InProgress);
}

#[test]
fn only_browser_scope_download_events_count() {
    assert!(is_download_event(&begin("g", "a")));
    assert!(is_download_start(&begin("g", "a")));
    assert!(!is_download_start(&progress("g", "inProgress", 0, None)));
    let mut page_scoped = begin("g", "a");
    page_scoped.session_id = Some("S".into());
    assert!(!is_download_event(&page_scoped));
    assert!(!is_download_event(&browser_event(
        "Page.loadEventFired",
        json!({})
    )));
}
