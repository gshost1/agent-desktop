use std::ffi::OsString;
use std::path::PathBuf;

#[cfg(target_os = "macos")]
const DEFAULT_LOCATIONS: &[&str] = &[
    "Google/Chrome",
    "Google/Chrome Canary",
    "Chromium",
    "Microsoft Edge",
    "BraveSoftware/Brave-Browser",
];

#[cfg(target_os = "linux")]
const DEFAULT_LOCATIONS: &[&str] = &[
    "google-chrome",
    "google-chrome-unstable",
    "chromium",
    "microsoft-edge",
    "BraveSoftware/Brave-Browser",
];

#[cfg(target_os = "windows")]
const DEFAULT_LOCATIONS: &[&str] = &[
    "Google/Chrome/User Data",
    "Google/Chrome SxS/User Data",
    "Chromium/User Data",
    "Microsoft/Edge/User Data",
    "BraveSoftware/Brave-Browser/User Data",
];

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
const DEFAULT_LOCATIONS: &[&str] = &[];

/// Every known browser's default user-data directory on this OS, whether or
/// not it exists.
pub(crate) fn default_user_data_dirs() -> Vec<PathBuf> {
    user_data_dirs_with(|name| std::env::var_os(name))
}

pub(crate) fn user_data_dirs_with(lookup: impl Fn(&str) -> Option<OsString>) -> Vec<PathBuf> {
    let Some(base) = base_dir(&lookup) else {
        return Vec::new();
    };
    DEFAULT_LOCATIONS
        .iter()
        .map(|relative| {
            relative
                .split('/')
                .fold(base.clone(), |path, part| path.join(part))
        })
        .collect()
}

fn non_empty(value: Option<OsString>) -> Option<PathBuf> {
    value.filter(|value| !value.is_empty()).map(PathBuf::from)
}

#[cfg(target_os = "macos")]
fn base_dir(lookup: &impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    non_empty(lookup("HOME")).map(|home| home.join("Library").join("Application Support"))
}

#[cfg(target_os = "linux")]
fn base_dir(lookup: &impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    non_empty(lookup("XDG_CONFIG_HOME"))
        .or_else(|| non_empty(lookup("HOME")).map(|home| home.join(".config")))
}

#[cfg(target_os = "windows")]
fn base_dir(lookup: &impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    non_empty(lookup("LOCALAPPDATA"))
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn base_dir(lookup: &impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    non_empty(lookup("HOME"))
}

#[cfg(test)]
#[path = "browser_user_data_dir_tests.rs"]
mod tests;
