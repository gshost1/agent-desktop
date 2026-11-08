use serde_json::Value;

use super::download_record::DownloadRecord;
use super::download_state::DownloadState;
use crate::CdpEvent;

const WILL_BEGIN: &str = "Browser.downloadWillBegin";
const PROGRESS: &str = "Browser.downloadProgress";

/// Whether `event` is a browser-scope download event.
pub(crate) fn is_download_event(event: &CdpEvent) -> bool {
    event.session_id.is_none() && (event.method == WILL_BEGIN || event.method == PROGRESS)
}

/// Whether `event` announces a new download.
pub(crate) fn is_download_start(event: &CdpEvent) -> bool {
    event.session_id.is_none() && event.method == WILL_BEGIN
}

/// The guid a download event carries.
pub(crate) fn event_guid(event: &CdpEvent) -> Option<&str> {
    event.params.get("guid").and_then(Value::as_str)
}

/// Folds one download event into `records`. A start adds a record once per
/// guid; progress updates its state, size and, once the browser reports
/// it, the saved path. A finished state never goes back to in progress, so
/// an event that arrives late cannot reopen a download.
pub(crate) fn apply_event(records: &mut Vec<DownloadRecord>, event: &CdpEvent) {
    let Some(guid) = event_guid(event) else {
        return;
    };
    let params = &event.params;
    if event.method == WILL_BEGIN {
        if records.iter().any(|record| record.guid == guid) {
            return;
        }
        records.push(DownloadRecord {
            guid: guid.to_owned(),
            url: text(params, "url"),
            suggested_filename: text(params, "suggestedFilename"),
            state: DownloadState::InProgress,
            bytes: 0,
            path: None,
            delivered: false,
        });
        return;
    }
    let Some(record) = records.iter_mut().find(|record| record.guid == guid) else {
        return;
    };
    if record.state != DownloadState::InProgress {
        return;
    }
    if let Some(bytes) = params.get("receivedBytes").and_then(Value::as_u64) {
        record.bytes = bytes;
    }
    if let Some(path) = params.get("filePath").and_then(Value::as_str) {
        record.path = Some(path.to_owned());
    }
    if let Some(state) = params
        .get("state")
        .and_then(Value::as_str)
        .and_then(DownloadState::from_cdp)
    {
        record.state = state;
    }
}

fn text(params: &Value, key: &str) -> String {
    params
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

#[cfg(test)]
#[path = "download_event_tests.rs"]
mod tests;
