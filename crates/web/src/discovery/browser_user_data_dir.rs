use std::ffi::OsString;
use std::path::PathBuf;

/// A browser's default user-data directory on this OS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserUserDataDir {
    pub browser: &'static str,
    pub path: PathBuf,
}

#[cfg(target_os = "macos")]
const DEFAULT_LOCATIONS: &[(&str, &str)] = &[
    ("chrome", "Google/Chrome"),
    ("chrome-canary", "Google/Chrome Canary"),
    ("chromium", "Chromium"),
    ("edge", "Microsoft Edge"),
    ("brave", "BraveSoftware/Brave-Browser"),
];

#[cfg(target_os = "linux")]
const DEFAULT_LOCATIONS: &[(&str, &str)] = &[
    ("chrome", "google-chrome"),
    ("chrome-canary", "google-chrome-unstable"),
    ("chromium", "chromium"),
    ("edge", "microsoft-edge"),
    ("brave", "BraveSoftware/Brave-Browser"),
];

#[cfg(target_os = "windows")]
const DEFAULT_LOCATIONS: &[(&str, &str)] = &[
    ("chrome", "Google/Chrome/User Data"),
    ("chrome-canary", "Google/Chrome SxS/User Data"),
    ("chromium", "Chromium/User Data"),
    ("edge", "Microsoft/Edge/User Data"),
    ("brave", "BraveSoftware/Brave-Browser/User Data"),
];

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
const DEFAULT_LOCATIONS: &[(&str, &str)] = &[];

/// Every known browser's default user-data directory on this OS, whether or
/// not it exists.
pub fn default_user_data_dirs() -> Vec<BrowserUserDataDir> {
    user_data_dirs_with(|name| std::env::var_os(name))
}

/// The default user-data directory of `browser` (`chrome`, `chrome-canary`,
/// `chromium`, `edge` or `brave`) on this OS.
pub fn default_user_data_dir(browser: &str) -> Option<PathBuf> {
    default_user_data_dirs()
        .into_iter()
        .find(|entry| entry.browser.eq_ignore_ascii_case(browser))
        .map(|entry| entry.path)
}

pub(crate) fn user_data_dirs_with(
    lookup: impl Fn(&str) -> Option<OsString>,
) -> Vec<BrowserUserDataDir> {
    let Some(base) = base_dir(&lookup) else {
        return Vec::new();
    };
    DEFAULT_LOCATIONS
        .iter()
        .map(|(browser, relative)| BrowserUserDataDir {
            browser,
            path: relative
                .split('/')
                .fold(base.clone(), |path, part| path.join(part)),
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
