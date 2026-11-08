use std::path::{Path, PathBuf};

use agent_desktop_core::{AdapterError, DownloadScope};
use serde_json::{Value, json};

use crate::discovery::browser_user_data_dir::default_user_data_dirs;

const USER_DATA_DIR: &str = "--user-data-dir=";
const PROFILE_DIRECTORY: &str = "--profile-directory=";
const DEFAULT_PROFILE: &str = "Default";
const MAX_PREFERENCES_BYTES: u64 = 32 * 1024 * 1024;

/// Writes `download_dir` into the download preference of the profile the
/// launch arguments name, before the browser starts, so the browser saves
/// every download there under its real name without asking, whether or not
/// a DevTools connection is open. Only an absolute `--user-data-dir` that is
/// not a browser's default user-data directory is written to, so a real
/// profile's downloads are never redirected; anything else, and a
/// `Preferences` file that is not a JSON object, is left as it is and
/// returns `Connection`.
pub(crate) fn save_in_profile(
    args: &[String],
    download_dir: &Path,
) -> Result<DownloadScope, AdapterError> {
    let defaults: Vec<PathBuf> = default_user_data_dirs()
        .into_iter()
        .map(|entry| entry.path)
        .collect();
    let Some(preferences) = preferences_path(args, &defaults) else {
        return Ok(DownloadScope::Connection);
    };
    let Some(mut document) = read_preferences(&preferences)? else {
        return Ok(DownloadScope::Connection);
    };
    point_downloads_at(&mut document, download_dir);
    write_preferences(&preferences, &document)?;
    Ok(DownloadScope::Profile)
}

/// The `Preferences` file of the profile `args` launch, or `None` when the
/// arguments name no user-data directory this may write to.
pub(crate) fn preferences_path(args: &[String], defaults: &[PathBuf]) -> Option<PathBuf> {
    let user_data_dir = PathBuf::from(
        args.iter()
            .rev()
            .find_map(|arg| arg.strip_prefix(USER_DATA_DIR))?,
    );
    if !user_data_dir.is_absolute() || defaults.iter().any(|path| same_dir(path, &user_data_dir)) {
        return None;
    }
    let profile = args
        .iter()
        .rev()
        .find_map(|arg| arg.strip_prefix(PROFILE_DIRECTORY))
        .unwrap_or(DEFAULT_PROFILE);
    let plain =
        !profile.is_empty() && profile != "." && profile != ".." && !profile.contains(['/', '\\']);
    plain.then(|| user_data_dir.join(profile).join("Preferences"))
}

fn same_dir(a: &Path, b: &Path) -> bool {
    a == b || matches!((a.canonicalize(), b.canonicalize()), (Ok(a), Ok(b)) if a == b)
}

fn read_preferences(path: &Path) -> Result<Option<Value>, AdapterError> {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.len() > MAX_PREFERENCES_BYTES => return Ok(None),
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Some(json!({}))),
        Err(error) => return Err(io_failure(path, &error)),
    }
    let bytes = std::fs::read(path).map_err(|error| io_failure(path, &error))?;
    Ok(serde_json::from_slice::<Value>(&bytes)
        .ok()
        .filter(Value::is_object))
}

pub(crate) fn point_downloads_at(document: &mut Value, download_dir: &Path) {
    let dir = download_dir.to_string_lossy().into_owned();
    set(document, "download", "default_directory", json!(dir));
    set(document, "download", "prompt_for_download", json!(false));
    set(document, "download", "directory_upgrade", json!(true));
    set(document, "savefile", "default_directory", json!(dir));
}

fn set(document: &mut Value, section: &str, key: &str, value: Value) {
    if !document[section].is_object() {
        document[section] = json!({});
    }
    document[section][key] = value;
}

fn write_preferences(path: &Path, document: &Value) -> Result<(), AdapterError> {
    let bytes =
        serde_json::to_vec(document).map_err(|error| AdapterError::internal(error.to_string()))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| io_failure(parent, &error))?;
    }
    let staged = path.with_extension("agent-desktop-staged");
    std::fs::write(&staged, bytes).map_err(|error| io_failure(&staged, &error))?;
    std::fs::rename(&staged, path).map_err(|error| io_failure(path, &error))
}

fn io_failure(path: &Path, error: &std::io::Error) -> AdapterError {
    AdapterError::internal(format!(
        "browser profile preferences {}: {error}",
        path.display()
    ))
}

#[cfg(test)]
#[path = "download_profile_tests.rs"]
mod tests;
