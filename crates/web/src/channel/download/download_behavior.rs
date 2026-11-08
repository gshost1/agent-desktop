use std::path::Path;

use agent_desktop_core::{AdapterError, Deadline};
use serde_json::json;

use crate::CdpConnection;

/// Turns on the browser's download events for this connection. With a
/// download folder, the browser saves each download there under its guid so
/// a later invocation can find it by name; without one the browser keeps its
/// own folder and naming, so a real profile's downloads are never redirected.
/// The browser resets this when the connection closes, so every connection
/// sets it again.
pub(crate) fn enable_downloads(
    connection: &mut CdpConnection,
    download_dir: Option<&Path>,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let params = match download_dir {
        Some(dir) => json!({
            "behavior": "allowAndName",
            "downloadPath": dir,
            "eventsEnabled": true,
        }),
        None => json!({ "behavior": "default", "eventsEnabled": true }),
    };
    connection
        .call("Browser.setDownloadBehavior", params, None, deadline)
        .map(|_| ())
}

#[cfg(test)]
#[path = "download_behavior_tests.rs"]
mod tests;
