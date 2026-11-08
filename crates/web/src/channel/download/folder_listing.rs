use std::path::Path;
use std::time::SystemTime;

use super::folder_file::FolderFile;

const PARTIAL_SUFFIX: &str = ".crdownload";

/// What a download folder holds: its finished files, oldest change first,
/// and the names of the partial files the browser is still writing. Hidden
/// files, folders and names that are not UTF-8 are left out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct FolderListing {
    pub(crate) finished: Vec<FolderFile>,
    pub(crate) partial: Vec<String>,
}

impl FolderListing {
    pub(crate) fn read(dir: &Path) -> Self {
        let mut listing = Self::default();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return listing;
        };
        for entry in entries.flatten() {
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            if name.starts_with('.') || !metadata.is_file() {
                continue;
            }
            if let Some(stem) = name.strip_suffix(PARTIAL_SUFFIX) {
                listing.partial.push(stem.to_owned());
                continue;
            }
            listing.finished.push(FolderFile {
                name,
                bytes: metadata.len(),
                modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            });
        }
        listing.finished.sort_by(|a, b| {
            a.modified
                .cmp(&b.modified)
                .then_with(|| a.name.cmp(&b.name))
        });
        listing
    }
}

/// Whether `name` is the file the browser saves for a download it was told
/// to call `suggested`: the name itself, or the name with ` (n)` before its
/// extension that the browser picks when the name is taken.
pub(crate) fn saved_as(suggested: &str, name: &str) -> bool {
    if name == suggested {
        return true;
    }
    let (stem, extension) = match suggested.rfind('.') {
        Some(dot) if dot > 0 => suggested.split_at(dot),
        _ => (suggested, ""),
    };
    let Some(rest) = name
        .strip_prefix(stem)
        .and_then(|rest| rest.strip_suffix(extension))
    else {
        return false;
    };
    rest.strip_prefix(" (")
        .and_then(|rest| rest.strip_suffix(')'))
        .is_some_and(|number| !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(test)]
#[path = "folder_listing_tests.rs"]
mod tests;
