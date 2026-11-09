use std::time::Instant;

use serde_json::{Value, json};

use crate::{
    AppError,
    adapter::PlatformAdapter,
    commands::{
        helpers::{process_identity, resolve_app},
        wait_timeout,
    },
};

/// Waits for the app's next download no earlier wait returned and reports
/// the saved file with the time the wait took.
pub(crate) fn wait_for_download(
    app: &str,
    timeout_ms: u64,
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AppError> {
    let start = Instant::now();
    let deadline = crate::Deadline::at(start, timeout_ms)?;
    let target = resolve_app(Some(app), adapter, deadline)?;
    let process = process_identity(&target)?;
    let download = adapter
        .wait_for_download(process, deadline)
        .map_err(|error| wait_timeout::download(error, timeout_ms))?;
    let mut value = serde_json::to_value(download)?;
    value["elapsed_ms"] = json!(start.elapsed().as_millis());
    Ok(value)
}

#[cfg(test)]
#[path = "wait_download_tests.rs"]
mod tests;
