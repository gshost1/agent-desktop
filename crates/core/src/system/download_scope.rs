use serde::{Deserialize, Serialize};

/// How a browser launched with `--download-dir` keeps saving there.
/// `Profile`: launch wrote the folder into the browser profile's own
/// download preference, so every download lands there under its real name
/// whether or not a DevTools connection is open. `Connection`: only a
/// DevTools connection points downloads at the folder, so a download that
/// begins after the command's connection closed lands in the browser's own
/// folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadScope {
    Profile,
    Connection,
}
