use std::path::Path;

use crate::AppError;

/// Resolves `launch --download-dir` to an absolute folder, creating it when
/// missing. The path is made absolute without resolving links, so a Windows
/// path never gains the `\\?\` prefix a browser would reject. A download
/// folder only takes effect through the DevTools endpoint, so it needs
/// `--cdp`.
pub(crate) fn prepare_download_dir(dir: &Path, cdp_port: Option<u16>) -> Result<String, AppError> {
    if cdp_port.is_none() {
        return Err(AppError::invalid_input_with_suggestion(
            "--download-dir requires --cdp",
            "Add --cdp so later commands can point the browser's downloads at the folder.",
        ));
    }
    let absolute = std::path::absolute(dir).map_err(|error| unusable(dir, &error.to_string()))?;
    std::fs::create_dir_all(&absolute).map_err(|error| unusable(dir, &error.to_string()))?;
    if !absolute.is_dir() {
        return Err(unusable(dir, "not a folder"));
    }
    absolute
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| unusable(dir, "the path is not valid UTF-8"))
}

fn unusable(dir: &Path, reason: &str) -> AppError {
    AppError::invalid_input_with_suggestion(
        format!("--download-dir {} cannot be used: {reason}", dir.display()),
        "Pass a folder this user can create and write to.",
    )
}

#[cfg(test)]
#[path = "download_dir_tests.rs"]
mod tests;
