use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode};
use serde_json::json;

use super::download_record::DownloadRecord;

/// The browser canceled the download, or the user did; no file was saved.
pub(crate) fn canceled(record: &DownloadRecord) -> AdapterError {
    failed(
        record,
        "cdp_download_canceled",
        "The browser canceled the download",
    )
    .with_suggestion("Start the download again, then wait --download")
}

/// The partial file vanished and no finished file replaced it.
pub(crate) fn interrupted(record: &DownloadRecord) -> AdapterError {
    failed(
        record,
        "cdp_download_interrupted",
        "The download stopped before it finished",
    )
    .with_suggestion("Start the download again, then wait --download")
}

/// A download that began in an earlier invocation of a browser with no
/// download folder: its progress went to a connection that has closed.
pub(crate) fn untracked(record: &DownloadRecord) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionNotSupported,
        "The download began in an earlier command and the browser has no download folder to read it from",
    )
    .with_suggestion(
        "Launch the browser with --cdp --download-dir <dir>, or run the click and wait --download in one batch",
    )
    .with_details(json!({
        "kind": "cdp_download_untracked",
        "url": record.url,
        "suggested_filename": record.suggested_filename,
    }))
    .with_disposition(DeliverySemantics::not_delivered())
}

/// No DevTools endpoint reaches the browser, so its downloads cannot be seen.
pub(crate) fn no_endpoint() -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionNotSupported,
        "No DevTools endpoint reaches this browser, so its downloads cannot be read",
    )
    .with_suggestion("Launch the browser with --cdp --download-dir <dir>, then download again")
    .with_details(json!({ "kind": "cdp_endpoint_missing" }))
    .with_disposition(DeliverySemantics::not_delivered())
}

/// The wait ran out: `pending` is the download still receiving, if any.
pub(crate) fn timed_out(pending: Option<&DownloadRecord>) -> AdapterError {
    let error = match pending {
        Some(record) => AdapterError::timeout("The download did not finish before the timeout")
            .with_details(known_url(json!({
                "kind": "cdp_download_in_progress",
                "url": record.url,
                "suggested_filename": record.suggested_filename,
                "bytes": record.bytes,
            }))),
        None => AdapterError::timeout("No download began before the timeout")
            .with_details(json!({ "kind": "cdp_download_not_started" })),
    };
    error.with_suggestion("Start the download, or wait again with a longer --timeout")
}

fn failed(record: &DownloadRecord, kind: &str, message: &str) -> AdapterError {
    AdapterError::new(ErrorCode::ActionFailed, message)
        .with_details(known_url(json!({
            "kind": kind,
            "url": record.url,
            "suggested_filename": record.suggested_filename,
            "bytes": record.bytes,
        })))
        .with_disposition(DeliverySemantics::not_delivered())
}

/// Drops `url` from error details when no connection saw where the download
/// came from.
fn known_url(mut details: serde_json::Value) -> serde_json::Value {
    if details["url"].as_str().is_some_and(str::is_empty)
        && let Some(object) = details.as_object_mut()
    {
        object.remove("url");
    }
    details
}
