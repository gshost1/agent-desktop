use std::path::{Path, PathBuf};

const PARTIAL_SUFFIX: &str = ".crdownload";
const FALLBACK_NAME: &str = "download";
const MAX_COPIES: u32 = 1000;

/// What the download folder shows for one guid-named download.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiskState {
    Finished(u64),
    Partial,
    Missing,
}

/// Reads the folder for the file the browser writes under `guid`: the
/// finished file, the partial `.crdownload` it writes while receiving, or
/// neither.
pub(crate) fn disk_state(dir: &Path, guid: &str) -> DiskState {
    let Some(name) = plain_name(guid) else {
        return DiskState::Missing;
    };
    if let Ok(metadata) = std::fs::metadata(dir.join(&name))
        && metadata.is_file()
    {
        return DiskState::Finished(metadata.len());
    }
    if dir.join(format!("{name}{PARTIAL_SUFFIX}")).is_file() {
        return DiskState::Partial;
    }
    DiskState::Missing
}

/// The guid-named file in `dir`, when `guid` is a plain file name.
pub(crate) fn guid_path(dir: &Path, guid: &str) -> Option<PathBuf> {
    plain_name(guid).map(|name| dir.join(name))
}

/// Moves the finished guid-named file to the server's suggested name in the
/// same folder, adding ` (n)` before the extension when that name is taken.
/// An existing file is never replaced: each candidate is claimed with a hard
/// link, which fails when the name exists, and the guid name is removed only
/// after the link succeeds. A folder without hard links falls back to a
/// rename once the name is seen to be free.
pub(crate) fn place(dir: &Path, guid: &str, suggested: &str) -> std::io::Result<PathBuf> {
    let source = guid_path(dir, guid).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "the guid is not a file name",
        )
    })?;
    let name = safe_file_name(suggested);
    for copy in 0..MAX_COPIES {
        let target = dir.join(numbered(&name, copy));
        match std::fs::hard_link(&source, &target) {
            Ok(()) => {
                std::fs::remove_file(&source)?;
                return Ok(target);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) if !target.exists() => {
                std::fs::rename(&source, &target)?;
                return Ok(target);
            }
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "every numbered copy of the name is taken",
    ))
}

/// A file name that stays inside the download folder on every platform:
/// the last path component only, with control characters and the
/// characters Windows reserves replaced, and no name that is empty or made
/// only of dots.
pub(crate) fn safe_file_name(suggested: &str) -> String {
    let last = suggested.rsplit(['/', '\\']).next().unwrap_or_default();
    let cleaned: String = last
        .chars()
        .map(|ch| {
            if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '|' | '?' | '*') {
                '_'
            } else {
                ch
            }
        })
        .collect();
    let trimmed = cleaned.trim_end_matches(['.', ' ']).trim_start();
    if trimmed.is_empty() || trimmed.chars().all(|ch| ch == '.') {
        return FALLBACK_NAME.to_owned();
    }
    if is_reserved_device(trimmed) {
        return format!("_{trimmed}");
    }
    trimmed.to_owned()
}

/// Windows device names, which name a device rather than a file in any
/// folder, whatever extension follows.
fn is_reserved_device(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit())
}

fn numbered(name: &str, copy: u32) -> String {
    if copy == 0 {
        return name.to_owned();
    }
    match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => format!("{stem} ({copy}).{extension}"),
        _ => format!("{name} ({copy})"),
    }
}

fn plain_name(guid: &str) -> Option<String> {
    let valid = !guid.is_empty()
        && guid
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-');
    valid.then(|| guid.to_owned())
}

#[cfg(test)]
#[path = "download_placement_tests.rs"]
mod tests;
