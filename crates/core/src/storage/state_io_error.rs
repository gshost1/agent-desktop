use crate::storage::state_exposure::StateExposure;
use crate::{AdapterError, ErrorCode};

/// Maps an io failure on agent-desktop's own state files. A path other users
/// can reach is `PERM_DENIED` with the path and the `chmod` that closes it,
/// because the caller can repair it and `INTERNAL` offers no action. Any
/// other permission refusal keeps `PERM_DENIED` without that advice, and the
/// rest stay `INTERNAL`.
pub(crate) fn state_io_error(error: std::io::Error) -> AdapterError {
    if let Some(exposure) = crate::storage::state_exposure::find(&error) {
        return exposure_error(exposure);
    }
    let code = if error.kind() == std::io::ErrorKind::PermissionDenied {
        ErrorCode::PermDenied
    } else {
        ErrorCode::Internal
    };
    AdapterError::new(code, error.to_string())
}

/// Keeps an exposure's own error and hands every other failure to the
/// caller's mapping.
pub(crate) fn exposure_or(
    error: std::io::Error,
    otherwise: impl FnOnce(std::io::Error) -> AdapterError,
) -> AdapterError {
    match crate::storage::state_exposure::find(&error) {
        Some(exposure) => exposure_error(exposure),
        None => otherwise(error),
    }
}

pub(crate) fn exposure_error(exposure: &StateExposure) -> AdapterError {
    let (subject, mode) = if exposure.directory {
        ("directory", "chmod 700")
    } else {
        ("file", "chmod 600")
    };
    let location = if exposure.path.is_some() {
        format!("the {subject} in details.path")
    } else {
        format!("the state {subject}")
    };
    let mut details = serde_json::json!({
        "kind": "state_directory_permission",
        "retryable": false,
    });
    if let Some(path) = &exposure.path {
        details["path"] = serde_json::Value::String(path.display().to_string());
    }
    AdapterError::new(ErrorCode::PermDenied, exposure.to_string())
        .with_suggestion(format!(
            "agent-desktop keeps its state private and refuses a path other users can reach. \
             Close {location} to them with {mode}, then retry"
        ))
        .with_details(details)
}

#[cfg(test)]
#[path = "state_io_error_tests.rs"]
mod tests;
