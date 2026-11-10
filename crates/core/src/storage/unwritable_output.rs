use crate::AppError;
use std::path::Path;

/// A write that fails at a path the caller named is a problem with that path,
/// not a missing OS permission, so it is an argument error rather than the
/// `PERM_DENIED` or `INTERNAL` the generic io mapping would give it.
pub(crate) fn unwritable_output(what: &str, path: &Path, error: AppError) -> AppError {
    match error {
        AppError::Io(io) => AppError::invalid_input_with_suggestion(
            format!("Cannot write {what} to '{}': {io}", path.display()),
            "Pass an output path in a directory this user can write to; missing parent directories are created",
        ),
        other => other,
    }
}
